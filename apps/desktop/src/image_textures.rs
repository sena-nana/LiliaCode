use std::collections::BTreeMap;

use nana_ui::{
    GpuContext, GpuTextureDescriptor, GpuTextureFormat, GpuTextureRegion, GpuTextureUsages,
    HostTexture, HostTextureAlphaMode, HostTextureRegistry,
};

use crate::markdown_images::ImagePixels;

#[derive(Default)]
pub(crate) struct ImageTextures {
    pub registry: HostTextureRegistry,
    sources: BTreeMap<String, String>,
}

impl ImageTextures {
    pub fn upload(&mut self, slot: &str, source: &str, pixels: &ImagePixels, gpu: &GpuContext) {
        if self
            .sources
            .get(slot)
            .is_some_and(|existing| existing == source)
        {
            return;
        }
        let limit = gpu.capabilities().max_texture_dimension_2d();
        let resized = if pixels.width > limit || pixels.height > limit {
            let image =
                image::RgbaImage::from_raw(pixels.width, pixels.height, pixels.rgba.to_vec())
                    .expect("decoded RGBA image");
            Some(
                image::DynamicImage::ImageRgba8(image)
                    .resize(limit, limit, image::imageops::FilterType::Triangle)
                    .to_rgba8(),
            )
        } else {
            None
        };
        let (width, height) = resized
            .as_ref()
            .map(|image| image.dimensions())
            .unwrap_or((pixels.width, pixels.height));
        let mut rgba = resized
            .map(|image| image.into_raw())
            .unwrap_or_else(|| pixels.rgba.to_vec());
        for pixel in rgba.chunks_exact_mut(4) {
            let alpha = pixel[3];
            for channel in &mut pixel[..3] {
                *channel = premultiply_srgb(*channel, alpha);
            }
        }
        let texture = gpu
            .create_texture(&GpuTextureDescriptor {
                label: Some("LiliaCode image preview"),
                width,
                height,
                format: GpuTextureFormat::RGBA8_UNORM_SRGB,
                usage: GpuTextureUsages::SAMPLED | GpuTextureUsages::COPY_DST,
            })
            .expect("image preview texture allocation");
        gpu.write_texture(
            &texture,
            GpuTextureRegion::full(width, height),
            &rgba,
            width * 4,
        )
        .expect("image preview texture upload");
        self.registry.register(
            slot,
            HostTexture::new(1, gpu.generation().get(), &texture),
            width,
            height,
            HostTextureAlphaMode::Premultiplied,
        );
        self.sources.insert(slot.to_owned(), source.to_owned());
    }

    pub fn retain(&mut self, slots: &[String]) {
        self.sources.retain(|slot, _| {
            if slots.contains(slot) {
                true
            } else {
                self.registry.remove(slot);
                false
            }
        });
    }

    pub fn invalidate(&mut self) {
        self.retain(&[]);
    }
}

fn premultiply_srgb(channel: u8, alpha: u8) -> u8 {
    if alpha == 255 {
        return channel;
    }
    let encoded = f32::from(channel) / 255.0;
    let linear = if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    };
    let linear = linear * f32::from(alpha) / 255.0;
    let encoded = if linear <= 0.0031308 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premultiplied_srgb_preserves_linear_light_and_transparency() {
        assert_eq!(premultiply_srgb(255, 128), 188);
        assert_eq!(premultiply_srgb(120, 255), 120);
        assert_eq!(premultiply_srgb(255, 0), 0);
    }
}
