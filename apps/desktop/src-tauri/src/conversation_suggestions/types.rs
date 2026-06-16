use serde::{Deserialize, Serialize};

pub(super) const SETTINGS_KEY: &str = "conversation-suggestions.settings";
pub(super) const CACHE_KEY: &str = "conversation-suggestions.cache";
pub(super) const CLAUDE_NATIVE_CACHE_KEY: &str = "conversation-suggestions.claude-native";
pub(super) const CACHE_TTL_MS: i64 = 24 * 60 * 60 * 1000;
pub(super) const MAX_TASKS_PER_SCOPE: usize = 3;
pub(super) const TASK_CANDIDATE_LIMIT: usize = 12;
pub(super) const MAX_SUGGESTIONS: usize = 3;
pub(super) const SAMPLE_TEXT_LIMIT: usize = 280;
pub(super) const SUMMARY_LIMIT: usize = 40;
pub(super) const REASON_LIMIT: usize = 120;
pub(super) const PROMPT_LIMIT: usize = 600;
pub(super) const UNFINISHED_SIGNAL_LIMIT: usize = 5;
pub(super) const GITHUB_EVENT_FETCH_LIMIT: usize = 30;
pub(super) const GITHUB_ACTIVITY_LIMIT: usize = 6;
pub(super) const LOCAL_GIT_COMMIT_LIMIT: usize = 3;
pub(super) const LOCAL_GIT_FILE_LIMIT: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SuggestionSource {
    Provider,
    AssistantAi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionSettings {
    pub(crate) enabled: bool,
    pub(crate) source: SuggestionSource,
}

impl Default for SuggestionSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            source: SuggestionSource::AssistantAi,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionItem {
    pub(crate) id: String,
    pub(crate) project_id: Option<String>,
    pub(crate) task_ids: Vec<String>,
    pub(crate) source: SuggestionItemSource,
    pub(crate) github_activities: Vec<SuggestionGitHubActivityRef>,
    #[serde(default)]
    pub(crate) local_git_contexts: Vec<SuggestionLocalGitContextRef>,
    pub(crate) summary: String,
    pub(crate) reason: String,
    pub(crate) prompt: String,
    pub(crate) generated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SuggestionItemSource {
    Task,
    Github,
    LocalGit,
    Claude,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionGitHubActivityRef {
    pub(crate) id: String,
    pub(crate) repo_full_name: String,
    pub(crate) kind: String,
    pub(crate) title: String,
    pub(crate) url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionLocalGitContextRef {
    pub(crate) id: String,
    pub(crate) branch: String,
    pub(crate) status: String,
    pub(crate) changed_files: Vec<String>,
    pub(crate) recent_commits: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionSourceProbe {
    pub(crate) sources: Vec<SuggestionItemSource>,
    pub(crate) local_git: Option<SuggestionLocalGitProbe>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuggestionLocalGitProbe {
    pub(crate) has_recent_commits: bool,
    pub(crate) has_changed_files: bool,
}

#[derive(Debug, Clone)]
pub(super) struct TaskSample {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) status: String,
    pub(super) project_id: Option<String>,
    pub(super) user_messages: Vec<String>,
    pub(super) assistant_message: Option<String>,
    pub(super) unfinished_signals: Vec<String>,
    pub(super) latest_updated_at: i64,
}

#[derive(Debug, Clone)]
pub(super) struct ProjectContext {
    pub(super) name: Option<String>,
    pub(super) cwd: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GitHubRepoRef {
    pub(super) owner: String,
    pub(super) name: String,
    pub(super) full_name: String,
}

#[derive(Debug, Clone)]
pub(super) struct GitHubActivitySample {
    pub(super) id: String,
    pub(super) repo_full_name: String,
    pub(super) kind: String,
    pub(super) action: String,
    pub(super) title: String,
    pub(super) url: Option<String>,
    pub(super) details: Vec<String>,
    pub(super) fingerprint: String,
}

#[derive(Debug, Clone)]
pub(super) struct LocalGitContextSample {
    pub(super) context: SuggestionLocalGitContextRef,
    pub(super) fingerprint: String,
}

#[derive(Debug, Clone)]
pub(super) struct SuggestionScope {
    pub(super) project_id: Option<String>,
    pub(super) project_name: Option<String>,
    pub(super) tasks: Vec<TaskSample>,
    pub(super) github_repo: Option<GitHubRepoRef>,
    pub(super) github_activities: Vec<GitHubActivitySample>,
    pub(super) local_git_contexts: Vec<LocalGitContextSample>,
    pub(super) latest_updated_at: i64,
}

#[derive(Debug, Clone)]
pub(super) struct ModelRequest {
    pub(super) source: SuggestionSource,
    pub(super) backend: Option<String>,
    pub(super) model: String,
    pub(super) base_url: String,
    pub(super) api_key: String,
}

pub(super) fn now_millis() -> i64 {
    crate::util::now_millis() as i64
}
