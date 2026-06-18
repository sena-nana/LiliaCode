use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub cwd: Option<String>,
    pub session_count: i64,
    pub sort_order: i64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
    pub project_id: Option<String>,
    pub session_id: String,
    pub title: String,
    pub title_source: String,
    pub status: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    pub depends_on: Vec<String>,
    pub sort_order: i64,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SidebarConversationSummaryRow {
    pub task_id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub title: String,
    pub created_at: i64,
    pub pinned: bool,
    pub route: String,
}

pub(super) struct NewTask<'a> {
    pub id: &'a str,
    pub project_id: Option<&'a str>,
    pub session_id: &'a str,
    pub title: &'a str,
    pub status: &'a str,
    pub created_at: i64,
    pub parent_id: Option<&'a str>,
    pub sort_order: i64,
    pub depends_on: &'a [String],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilestoneRow {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub due_date: Option<i64>,
    pub order: i64,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMilestoneLinkRow {
    pub task_id: String,
    pub milestone_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRoadmapRow {
    pub milestones: Vec<MilestoneRow>,
    pub links: Vec<TaskMilestoneLinkRow>,
}
