use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use base64::Engine as _;
use reqwest::blocking::Client;
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, USER_AGENT};
use url::Url;

const MAX_MARKDOWN_IMAGE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_MARKDOWN_IMAGE_RESIDENT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) enum MarkdownImageLoadState {
    Pending { requested: bool },
    Loading,
    Ready(LoadedMarkdownImage),
    Evicted,
    Failed,
}

impl MarkdownImageLoadState {
    pub(crate) fn pending_request(&self) -> Option<bool> {
        match self {
            Self::Pending { requested } => Some(*requested),
            _ => None,
        }
    }

    pub(crate) fn request(&mut self) -> bool {
        if matches!(self, Self::Loading | Self::Ready(_)) {
            return false;
        }
        *self = Self::Pending { requested: true };
        true
    }
}

pub(crate) fn resident_image_bytes(images: &BTreeMap<String, MarkdownImageLoadState>) -> usize {
    images
        .values()
        .filter_map(|state| match state {
            MarkdownImageLoadState::Ready(image) => Some(image.resident_len()),
            _ => None,
        })
        .fold(0usize, usize::saturating_add)
}

pub(crate) fn evict_lru_image(
    images: &mut BTreeMap<String, MarkdownImageLoadState>,
    recency: &BTreeMap<String, u64>,
    protected: &BTreeSet<&str>,
) -> bool {
    let candidate = images
        .iter()
        .filter(|(source, state)| {
            matches!(state, MarkdownImageLoadState::Ready(_))
                && !protected.contains(source.as_str())
        })
        .min_by_key(|(source, _)| recency.get(source.as_str()).copied().unwrap_or_default())
        .map(|(source, _)| source.clone());
    let Some(source) = candidate else {
        return false;
    };
    images.insert(source, MarkdownImageLoadState::Evicted);
    true
}

pub(crate) fn admit_loaded_image(
    images: &mut BTreeMap<String, MarkdownImageLoadState>,
    recency: &BTreeMap<String, u64>,
    protected: &BTreeSet<&str>,
    source: String,
    image: LoadedMarkdownImage,
    budget: usize,
) -> bool {
    if image.resident_len() > budget {
        images.insert(source, MarkdownImageLoadState::Failed);
        return false;
    }
    while resident_image_bytes(images).saturating_add(image.resident_len()) > budget {
        if !evict_lru_image(images, recency, protected) {
            images.insert(source, MarkdownImageLoadState::Evicted);
            return false;
        }
    }
    images.insert(source, MarkdownImageLoadState::Ready(image));
    true
}

#[derive(Clone, Debug)]
pub(crate) struct LoadedMarkdownImage {
    bytes: Arc<[u8]>,
    media_type: String,
    data_url: OnceLock<Arc<str>>,
    pub(crate) pixels: ImagePixels,
}

#[derive(Clone, Debug)]
pub(crate) struct ImagePixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl LoadedMarkdownImage {
    pub(crate) fn data_url(&self) -> Arc<str> {
        self.data_url
            .get_or_init(|| {
                Arc::from(format!(
                    "data:{};base64,{}",
                    self.media_type,
                    base64::engine::general_purpose::STANDARD.encode(&self.bytes)
                ))
            })
            .clone()
    }

    pub(crate) fn media_type(&self) -> &str {
        &self.media_type
    }

    pub(crate) fn resident_len(&self) -> usize {
        self.bytes
            .len()
            .saturating_add(self.pixels.rgba.len())
            .saturating_add(self.data_url.get().map_or_else(
                || encoded_data_url_len(self.bytes.len(), self.media_type.len()),
                |url| url.len(),
            ))
    }

    #[cfg(test)]
    pub(crate) fn encoded_len(&self) -> usize {
        self.bytes.len()
    }
}

pub(crate) fn load_markdown_image(source: &str) -> Result<LoadedMarkdownImage, String> {
    let source = source.trim();
    if source.is_empty() {
        return Err("image source is empty".to_owned());
    }
    if source.starts_with("data:") {
        return load_data_image(source);
    }
    if let Ok(url) = Url::parse(source) {
        return match url.scheme() {
            "http" | "https" => load_remote_image(url),
            "file" => url
                .to_file_path()
                .map_err(|_| "file image URL is invalid".to_owned())
                .and_then(|path| load_file_image(&path)),
            _ => Err("image URL scheme is not supported".to_owned()),
        };
    }
    let path = PathBuf::from(source);
    if path.is_absolute() {
        load_file_image(&path)
    } else {
        Err("relative image paths need an application base directory".to_owned())
    }
}

fn load_remote_image(url: Url) -> Result<LoadedMarkdownImage, String> {
    if !url.username().is_empty() || url.password().is_some() {
        return Err("image URL credentials are not supported".to_owned());
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|error| format!("image client initialization failed: {error}"))?;
    let mut response = client
        .get(url)
        .header(USER_AGENT, "LiliaCode-Native/markdown-image")
        .send()
        .map_err(|error| format!("image request failed: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("image request returned {}", response.status()));
    }
    if response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .is_some_and(|length| length > MAX_MARKDOWN_IMAGE_BYTES)
    {
        return Err("image response is too large".to_owned());
    }
    let declared_media_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(normalize_image_media_type);
    let bytes = read_bounded(&mut response)?;
    let media_type = declared_media_type
        .or_else(|| detect_image_media_type(&bytes))
        .ok_or_else(|| "image response content type is not supported".to_owned())?;
    loaded_image(bytes, media_type)
}

fn load_data_image(source: &str) -> Result<LoadedMarkdownImage, String> {
    let (metadata, payload) = source
        .strip_prefix("data:")
        .and_then(|value| value.split_once(','))
        .ok_or_else(|| "image data URL is invalid".to_owned())?;
    let mut parts = metadata.split(';');
    let media_type = parts
        .next()
        .and_then(normalize_image_media_type)
        .ok_or_else(|| "image data URL content type is not supported".to_owned())?;
    if !parts.any(|part| part.eq_ignore_ascii_case("base64")) {
        return Err("image data URL must use base64 encoding".to_owned());
    }
    let estimated = payload.len().saturating_mul(3) / 4;
    if estimated as u64 > MAX_MARKDOWN_IMAGE_BYTES {
        return Err("image data URL is too large".to_owned());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .map_err(|_| "image data URL base64 is invalid".to_owned())?;
    loaded_image(bytes, media_type)
}

fn load_file_image(path: &Path) -> Result<LoadedMarkdownImage, String> {
    let metadata =
        std::fs::metadata(path).map_err(|error| format!("image file metadata failed: {error}"))?;
    if !metadata.is_file() {
        return Err("image path is not a file".to_owned());
    }
    if metadata.len() > MAX_MARKDOWN_IMAGE_BYTES {
        return Err("image file is too large".to_owned());
    }
    let media_type = path
        .extension()
        .and_then(|value| value.to_str())
        .and_then(image_media_type_for_extension)
        .ok_or_else(|| "image file type is not supported".to_owned())?;
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("image file could not be opened: {error}"))?;
    let bytes = read_bounded(&mut file)?;
    loaded_image(bytes, media_type)
}

fn read_bounded(reader: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_MARKDOWN_IMAGE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("image body could not be read: {error}"))?;
    if bytes.len() as u64 > MAX_MARKDOWN_IMAGE_BYTES {
        return Err("image body is too large".to_owned());
    }
    Ok(bytes)
}

fn encoded_data_url_len(bytes: usize, media_type: usize) -> usize {
    bytes
        .div_ceil(3)
        .saturating_mul(4)
        .saturating_add(media_type)
        .saturating_add("data:;base64,".len())
}

fn loaded_image(bytes: Vec<u8>, media_type: String) -> Result<LoadedMarkdownImage, String> {
    if bytes.is_empty() || !payload_matches_media_type(&bytes, &media_type) {
        return Err("image body does not match its content type".to_owned());
    }
    let encoding_bytes = bytes
        .len()
        .saturating_add(encoded_data_url_len(bytes.len(), media_type.len()));
    let pixel_budget = MAX_MARKDOWN_IMAGE_RESIDENT_BYTES.saturating_sub(encoding_bytes);
    let pixels = if let Some(format) = raster_image_format(&media_type) {
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(16_384);
        limits.max_image_height = Some(16_384);
        limits.max_alloc = Some(pixel_budget as u64);
        let mut reader = image::ImageReader::with_format(Cursor::new(&bytes), format);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|_| "image body could not be decoded".to_owned())?
            .into_rgba8();
        ImagePixels {
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw().into(),
        }
    } else {
        let image = nana_svg_raster::rasterize_document_capped(&bytes, 4096)
            .ok_or_else(|| "SVG image could not be decoded".to_owned())?;
        ImagePixels {
            width: image.width,
            height: image.height,
            rgba: image.rgba,
        }
    };
    if pixels.rgba.len() > pixel_budget {
        return Err("image exceeds the 64 MiB resident memory limit".to_owned());
    }
    let image = LoadedMarkdownImage {
        bytes: Arc::from(bytes),
        data_url: OnceLock::new(),
        media_type,
        pixels,
    };
    image.data_url();
    Ok(image)
}

fn raster_image_format(media_type: &str) -> Option<image::ImageFormat> {
    match media_type {
        "image/png" => Some(image::ImageFormat::Png),
        "image/jpeg" => Some(image::ImageFormat::Jpeg),
        "image/gif" => Some(image::ImageFormat::Gif),
        "image/webp" => Some(image::ImageFormat::WebP),
        "image/bmp" => Some(image::ImageFormat::Bmp),
        _ => None,
    }
}

fn normalize_image_media_type(value: &str) -> Option<String> {
    let value = value.split(';').next()?.trim().to_ascii_lowercase();
    matches!(
        value.as_str(),
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp" | "image/svg+xml"
    )
    .then_some(value)
}

fn image_media_type_for_extension(extension: &str) -> Option<String> {
    let media_type = match extension.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        _ => return None,
    };
    Some(media_type.to_owned())
}

fn detect_image_media_type(bytes: &[u8]) -> Option<String> {
    [
        "image/png",
        "image/jpeg",
        "image/gif",
        "image/webp",
        "image/bmp",
        "image/svg+xml",
    ]
    .into_iter()
    .find(|media_type| payload_matches_media_type(bytes, media_type))
    .map(str::to_owned)
}

fn payload_matches_media_type(bytes: &[u8], media_type: &str) -> bool {
    match media_type {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "image/bmp" => bytes.starts_with(b"BM"),
        "image/svg+xml" => {
            let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]);
            let prefix = prefix.trim_start_matches('\u{feff}').trim_start();
            prefix.starts_with("<svg") || (prefix.starts_with("<?xml") && prefix.contains("<svg"))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_images_are_bounded_and_verified_against_the_declared_type() {
        let png = concat!(
            "data:image/png;base64,",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
        );
        let loaded = load_markdown_image(png).unwrap();
        assert_eq!(loaded.media_type(), "image/png");
        assert!(loaded.encoded_len() > 0);
        assert_eq!((loaded.pixels.width, loaded.pixels.height), (1, 1));
        assert_eq!(loaded.pixels.rgba.len(), 4);
        assert!(Arc::ptr_eq(&loaded.data_url(), &loaded.data_url()));
        assert!(Arc::ptr_eq(&loaded.data_url(), &loaded.clone().data_url()));
        assert_eq!(loaded.data_url().as_ref(), png);

        assert!(
            load_markdown_image("data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB").is_err()
        );
        assert!(load_markdown_image("data:image/png;base64,PGh0bWw+").is_err());
        assert!(load_markdown_image("javascript:alert(1)").is_err());
    }

    fn cache_image() -> LoadedMarkdownImage {
        load_markdown_image(concat!("data:image/png;base64,",
            "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")).unwrap()
    }

    #[test]
    fn resident_budget_includes_encoded_pixels_and_cached_data_url() {
        let image = cache_image();
        assert_eq!(
            image.resident_len(),
            image.bytes.len() + image.pixels.rgba.len() + image.data_url().len()
        );
        assert!(image.resident_len() > image.encoded_len() * 2);
        let images =
            BTreeMap::from([("image".into(), MarkdownImageLoadState::Ready(image.clone()))]);
        assert_eq!(resident_image_bytes(&images), image.resident_len());
        assert!(image.resident_len() <= MAX_MARKDOWN_IMAGE_RESIDENT_BYTES);
    }

    #[test]
    fn image_lru_eviction_stays_idle_until_user_requests_it() {
        let image = cache_image();
        let budget = image.resident_len() * 2;
        let mut images = BTreeMap::from([
            ("old".into(), MarkdownImageLoadState::Ready(image.clone())),
            (
                "recent".into(),
                MarkdownImageLoadState::Ready(image.clone()),
            ),
            ("incoming".into(), MarkdownImageLoadState::Loading),
        ]);
        let mut recency = BTreeMap::from([
            ("old".into(), 1),
            ("recent".into(), 2),
            ("incoming".into(), 3),
        ]);
        assert!(admit_loaded_image(
            &mut images,
            &recency,
            &BTreeSet::new(),
            "incoming".into(),
            image.clone(),
            budget
        ));
        assert!(matches!(images["old"], MarkdownImageLoadState::Evicted));
        assert!(images
            .values()
            .all(|state| state.pending_request().is_none()));
        assert_eq!(resident_image_bytes(&images), budget);
        assert!(images.get_mut("old").unwrap().request());
        assert_eq!(images["old"].pending_request(), Some(true));
        images.insert("old".into(), MarkdownImageLoadState::Loading);
        recency.insert("old".into(), 4);
        assert!(admit_loaded_image(
            &mut images,
            &recency,
            &BTreeSet::new(),
            "old".into(),
            image,
            budget
        ));
        assert!(matches!(images["recent"], MarkdownImageLoadState::Evicted));
        assert!(matches!(
            images["incoming"],
            MarkdownImageLoadState::Ready(_)
        ));
        assert_eq!(resident_image_bytes(&images), budget);
    }

    #[test]
    fn oversized_image_does_not_flush_usable_cache_entries() {
        let image = cache_image();
        let budget = image.resident_len();
        let mut oversized = image.clone();
        oversized.pixels.rgba = vec![0; budget].into();
        let mut images = BTreeMap::from([
            ("keep".into(), MarkdownImageLoadState::Ready(image)),
            ("large".into(), MarkdownImageLoadState::Loading),
        ]);
        assert!(!admit_loaded_image(
            &mut images,
            &BTreeMap::new(),
            &BTreeSet::new(),
            "large".into(),
            oversized,
            budget
        ));
        assert!(matches!(images["large"], MarkdownImageLoadState::Failed));
        assert!(matches!(images["keep"], MarkdownImageLoadState::Ready(_)));
        assert_eq!(resident_image_bytes(&images), budget);
        assert_eq!(images["large"].pending_request(), None);
        assert!(images.get_mut("large").unwrap().request());
    }

    #[test]
    fn protected_preview_defers_other_image_without_reloading_loop() {
        let image = cache_image();
        let budget = image.resident_len();
        let mut images = BTreeMap::from([
            (
                "preview".into(),
                MarkdownImageLoadState::Ready(image.clone()),
            ),
            ("other".into(), MarkdownImageLoadState::Loading),
        ]);
        let recency = BTreeMap::from([("preview".into(), 1), ("other".into(), 2)]);
        assert!(!admit_loaded_image(
            &mut images,
            &recency,
            &BTreeSet::from(["preview"]),
            "other".into(),
            image.clone(),
            budget
        ));
        assert!(matches!(
            images["preview"],
            MarkdownImageLoadState::Ready(_)
        ));
        assert!(images
            .values()
            .all(|state| state.pending_request().is_none()));
        assert!(images.get_mut("other").unwrap().request());
        images.insert("other".into(), MarkdownImageLoadState::Loading);
        assert!(admit_loaded_image(
            &mut images,
            &recency,
            &BTreeSet::new(),
            "other".into(),
            image,
            budget
        ));
        assert!(matches!(images["preview"], MarkdownImageLoadState::Evicted));
        assert_eq!(resident_image_bytes(&images), budget);
    }

    #[test]
    fn svg_preview_contains_painted_pixels() {
        let image = loaded_image(br##"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="2"><rect width="4" height="2" fill="#ff0000"/></svg>"##.to_vec(), "image/svg+xml".into()).unwrap();
        assert_eq!((image.pixels.width, image.pixels.height), (4, 2));
        assert!(image
            .pixels
            .rgba
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 0, 0, 255]));
    }
}
