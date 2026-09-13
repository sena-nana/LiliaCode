use serde::{Deserialize, Serialize};

pub const TERMINAL_GRID_CONTRACT_JSON: &str =
    include_str!("../contracts/terminal-grid-contract.json");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TerminalCellColor {
    #[default]
    Default,
    Indexed(u8),
    Rgb([u8; 3]),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalGridCell {
    pub text: String,
    pub width: u8,
    pub foreground: TerminalCellColor,
    pub background: TerminalCellColor,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}
