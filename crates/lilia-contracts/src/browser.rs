use serde::{Deserialize, Serialize};
use std::fmt;

use crate::{ProjectId, TaskId};

pub const BROWSER_CONTRACT_JSON: &str = include_str!("../contracts/browser-contract.json");

/// Product-facing browser resource references are opaque handles, never file
/// paths or URLs carrying navigation data. Keep this check deliberately strict
/// because the value crosses the Product/Agent boundary.
pub fn is_opaque_browser_resource_ref(value: &str) -> bool {
    let Some(path) = value.strip_prefix("resource://browser/") else {
        return false;
    };
    !path.is_empty()
        && !path.starts_with('/')
        && !path.ends_with('/')
        && path.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && !segment.chars().any(|character| {
                    character.is_whitespace() || matches!(character, '\\' | '?' | '#')
                })
        })
}

#[cfg(test)]
mod resource_ref_tests {
    use super::is_opaque_browser_resource_ref;

    #[test]
    fn opaque_browser_resource_ref_rejects_paths_and_navigation_data() {
        assert!(is_opaque_browser_resource_ref(
            "resource://browser/project/task/hash.png"
        ));
        for value in [
            "resource://browser/",
            "resource://browser//task/hash.png",
            "resource://browser/../task/hash.png",
            "resource://browser/task/hash.png?token=secret",
            "resource://browser/task/hash.png#fragment",
            "resource://browser/task/hash\\.png",
            "/absolute/path/screenshot.png",
        ] {
            assert!(!is_opaque_browser_resource_ref(value), "{value}");
        }
    }

    #[test]
    fn browser_page_debug_does_not_include_navigation_secrets_or_page_text() {
        let page = super::BrowserPage {
            url: "https://alice:password@example.test/form?token=secret#fragment".into(),
            title: "typed secret".into(),
            targets: vec![super::BrowserTarget {
                id: "field".into(),
                role: "textbox".into(),
                name: "typed secret".into(),
            }],
            screenshot_artifact: None,
            screenshot_bytes: Some(vec![1, 2, 3]),
        };
        let debug = format!("{page:?}");
        assert!(!debug.contains("password"));
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("typed"));
        assert!(debug.contains("example.test/form"));
        assert!(debug.contains("screenshot_bytes_len"));
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserScope {
    pub project_id: ProjectId,
    pub task_id: TaskId,
    pub tab_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserTabRestoration {
    pub scope: BrowserScope,
    pub url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowserControl {
    Agent,
    Human,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserOperation {
    Observe,
    Navigate { url: String },
    Back,
    Forward,
    Reload,
    Click { target: String },
    Type { target: String, text: String },
    Scroll { x: i32, y: i32 },
    Screenshot,
}

impl std::fmt::Debug for BrowserOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Observe => "Observe",
            Self::Navigate { .. } => "Navigate",
            Self::Back => "Back",
            Self::Forward => "Forward",
            Self::Reload => "Reload",
            Self::Click { .. } => "Click",
            Self::Type { .. } => "Type([REDACTED])",
            Self::Scroll { .. } => "Scroll",
            Self::Screenshot => "Screenshot",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRequest {
    pub scope: BrowserScope,
    pub lifecycle: u64,
    pub page_version: u64,
    pub operation: BrowserOperation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTarget {
    pub id: String,
    pub role: String,
    pub name: String,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPage {
    pub url: String,
    pub title: String,
    pub targets: Vec<BrowserTarget>,
    /// Opaque product resource reference returned after artifact materialization.
    pub screenshot_artifact: Option<String>,
    /// Host-only PNG bytes awaiting application artifact materialization.
    /// This is intentionally omitted from all wire/state serialization.
    #[serde(skip)]
    pub screenshot_bytes: Option<Vec<u8>>,
}

impl fmt::Debug for BrowserPage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BrowserPage")
            .field("url", &debug_safe_url(&self.url))
            .field("title_len", &self.title.len())
            .field("target_count", &self.targets.len())
            .field("screenshot_artifact", &self.screenshot_artifact)
            .field(
                "screenshot_bytes_len",
                &self.screenshot_bytes.as_ref().map(Vec::len),
            )
            .finish()
    }
}

fn debug_safe_url(value: &str) -> String {
    let without_navigation = value
        .char_indices()
        .find(|(_, character)| matches!(character, '?' | '#'))
        .map_or(value, |(index, _)| &value[..index]);
    let Some(authority_start) = without_navigation.find("://").map(|index| index + 3) else {
        return without_navigation.to_owned();
    };
    let authority_end = without_navigation[authority_start..]
        .find('/')
        .map_or(without_navigation.len(), |index| authority_start + index);
    let authority = &without_navigation[authority_start..authority_end];
    let host = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    format!(
        "{}{}{}",
        &without_navigation[..authority_start],
        host,
        &without_navigation[authority_end..]
    )
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserState {
    pub scope: BrowserScope,
    pub lifecycle: u64,
    pub page_version: u64,
    pub control: BrowserControl,
    pub page: BrowserPage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserHostRequestKind {
    Download { suggested_filename: String },
    Upload { multiple: bool },
    NewWindow { url: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserHostRequest {
    pub id: u64,
    pub scope: BrowserScope,
    pub lifecycle: u64,
    pub page_version: u64,
    pub kind: BrowserHostRequestKind,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BrowserHostDecision {
    Reject,
    Download { path: String },
    Upload { paths: Vec<String> },
    NewWindow { target_scope: BrowserScope },
}

impl std::fmt::Debug for BrowserHostDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Reject => "Reject",
            Self::Download { .. } => "Download([HOST PATH])",
            Self::Upload { .. } => "Upload([HOST PATHS])",
            Self::NewWindow { .. } => "NewWindow",
        })
    }
}
