use crate::module::settings::presentation::{CustomAgentRow, McpEditor, McpRow, SkillRow};
use crate::runtime_shell::*;

pub trait Projection<'a> {
    fn fields(snapshot: &'a mut PrimaryShellSnapshot) -> Self;
}

macro_rules! projection {
    ($name:ident { $($field:ident : $ty:ty => $($path:ident).+),* $(,)? }) => {
        pub struct $name<'a> {
            $(pub $field: &'a mut $ty,)*
        }

        impl<'a> Projection<'a> for $name<'a> {
            fn fields(snapshot: &'a mut PrimaryShellSnapshot) -> Self {
                Self { $($field: &mut snapshot.$($path).+,)* }
            }
        }
    };
}

projection!(ArchitectureProjection {
    architecture: crate::module::architecture::view::ArchitectureViewSnapshot => architecture,
});

projection!(AutomationProjection {
    automation: crate::module::automation::view::AutomationViewSnapshot => automation,
});

projection!(ComposerProjection {
    composer: crate::module::composer::view::ComposerViewSnapshot => composer,
    error: Option<String> => error,
});

projection!(DocumentsProjection {
    document: Option<ShellDocumentSnapshot> => document,
});

projection!(ExtensionsProjection {
    can_create_skill: bool => settings.can_create_skill,
    extensions: Option<crate::runtime_extensions::ExtensionBrowserSnapshot> => settings.extensions,
    extensions_status: String => settings.extensions_status,
    mcp_editor: Option<McpEditor> => settings.mcp_editor,
    mcp_servers: Vec<McpRow> => settings.mcp_servers,
    skill_description: String => settings.skill_description,
    skill_id: String => settings.skill_id,
    skills: Vec<SkillRow> => settings.skills,
});

projection!(MemoryProjection {
    memory: crate::module::memory::view::MemoryViewSnapshot => memory,
});

projection!(RoadmapProjection {
    roadmap: crate::module::roadmap::view::RoadmapViewSnapshot => roadmap,
});

projection!(SettingsProjection {
    agent_actions: Vec<ShellActionRow> => settings.agent_actions,
    custom_agent_description: String => settings.custom_agent_description,
    custom_agent_editor_open: bool => settings.custom_agent_editor_open,
    custom_agent_instruction: String => settings.custom_agent_instruction,
    custom_agent_name: String => settings.custom_agent_name,
    custom_agents: Vec<CustomAgentRow> => settings.custom_agents,
    shortcut: String => settings.shortcut,
    shortcut_capturing: bool => settings.shortcut_capturing,
});

projection!(TaskProjection {
    tasks: Vec<ShellTaskRow> => tasks,
    session_search: String => session_search,
    session_page: usize => session_page,
    session_page_count: usize => session_page_count,
    session_cards: Vec<ShellTaskRow> => session_cards,
});

projection!(TimelineProjection {
    timeline: crate::module::timeline::view::TimelineViewSnapshot => timeline,
});
