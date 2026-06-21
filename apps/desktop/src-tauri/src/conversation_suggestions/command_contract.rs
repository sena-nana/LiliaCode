#[cfg(test)]
mod tests {
    use super::super::contract;
    use crate::conversation_suggestions::{
        conversation_suggestions_get, conversation_suggestions_get_settings,
        conversation_suggestions_get_sources, conversation_suggestions_set_settings,
    };

    #[test]
    fn conversation_suggestion_command_names_load_from_contract_manifest() {
        let commands = contract::commands();
        let _ = conversation_suggestions_get;
        let _ = conversation_suggestions_get_sources;
        let _ = conversation_suggestions_get_settings;
        let _ = conversation_suggestions_set_settings;

        assert_eq!(commands.get, stringify!(conversation_suggestions_get));
        assert_eq!(
            commands.get_sources,
            stringify!(conversation_suggestions_get_sources)
        );
        assert_eq!(
            commands.get_settings,
            stringify!(conversation_suggestions_get_settings)
        );
        assert_eq!(
            commands.set_settings,
            stringify!(conversation_suggestions_set_settings)
        );
    }
}
