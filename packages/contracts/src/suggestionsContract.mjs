import suggestionsContract from "./suggestions-contract.json" with { type: "json" };

const manifest = Object.freeze(suggestionsContract);

export const SUGGESTIONS_CONTRACT = manifest;
export const SUGGESTION_SOURCES = manifest.suggestionSources;
export const DEFAULT_SUGGESTION_SOURCE = manifest.defaultSuggestionSource;
export const CONVERSATION_SUGGESTIONS_GET_COMMAND = manifest.commands.get;
export const CONVERSATION_SUGGESTIONS_GET_SOURCES_COMMAND =
  manifest.commands.getSources;
export const CONVERSATION_SUGGESTIONS_GET_SETTINGS_COMMAND =
  manifest.commands.getSettings;
export const CONVERSATION_SUGGESTIONS_SET_SETTINGS_COMMAND =
  manifest.commands.setSettings;
export const SUGGESTION_SOURCE_LABELS = manifest.suggestionSourceLabels;
export const SUGGESTION_SOURCE_SETTING_ORDER =
  manifest.suggestionSourceSettingOrder;
export const SUGGESTION_ITEM_SOURCES = manifest.suggestionItemSources;
export const SUGGESTION_LOADING_SOURCE_LABELS =
  manifest.suggestionLoadingSourceLabels;
export const SUGGESTION_LOCAL_GIT_LOADING_LABELS =
  manifest.suggestionLocalGitLoadingLabels;
export const SUGGESTION_STATUSES = manifest.suggestionStatuses;
export const DEFAULT_SUGGESTION_STATUS = manifest.defaultSuggestionStatus;
export const DEFAULT_SUGGESTION_LOADING_TEXT =
  manifest.defaultSuggestionLoadingText;
export const SUGGESTION_STATUS_DISPLAY_TEXT =
  manifest.suggestionStatusDisplayText;
