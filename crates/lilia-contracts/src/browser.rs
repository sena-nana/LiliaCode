use serde::{Deserialize, Serialize};

use crate::{ProjectId, TaskId};

pub const BROWSER_CONTRACT_JSON: &str = include_str!("../contracts/browser-contract.json");

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserPage {
    pub url: String,
    pub title: String,
    pub targets: Vec<BrowserTarget>,
    pub screenshot_artifact: Option<String>,
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
