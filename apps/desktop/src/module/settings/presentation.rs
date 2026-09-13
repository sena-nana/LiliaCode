#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRow {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CredentialRow {
    pub id: String,
    pub revision: u64,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CustomAgentRow {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SkillRow {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpRow {
    pub id: String,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpEditor {
    pub server_id: String,
    pub transport: String,
    pub location: String,
    pub args: String,
    pub enabled: bool,
}
