pub(crate) mod code_checking;
pub(crate) mod search_locoryn_conversations;
pub(crate) mod web_search;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct ToolSettings {
    /// Master toggle: when false, no tools are offered to the model at all.
    pub enabled: bool,
    pub web_search: bool,
    pub fetch_webpage: bool,
    pub conversation_search: bool,
    /// Disabled by default because it invokes local compilers/interpreters.
    pub code_checking: bool,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            web_search: true,
            fetch_webpage: true,
            conversation_search: true,
            code_checking: false,
        }
    }
}

impl ToolSettings {
    pub fn any_tool_enabled(&self) -> bool {
        self.enabled
            && (self.web_search
                || self.fetch_webpage
                || self.conversation_search
                || self.code_checking)
    }

    pub fn web_tools_enabled(&self) -> bool {
        self.enabled && (self.web_search || self.fetch_webpage)
    }

    /// Applies the per-conversation Web switch without changing the global
    /// tool preference or local-only tools such as conversation search.
    pub fn for_chat_web_enabled(mut self, web_enabled: bool) -> Self {
        if !web_enabled {
            self.web_search = false;
            self.fetch_webpage = false;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::ToolSettings;

    #[test]
    fn disabling_web_for_a_chat_keeps_local_tools_available() {
        let settings = ToolSettings::default().for_chat_web_enabled(false);

        assert!(settings.enabled);
        assert!(!settings.web_search);
        assert!(!settings.fetch_webpage);
        assert!(settings.conversation_search);
        assert!(settings.any_tool_enabled());
        assert!(!settings.web_tools_enabled());
    }

    #[test]
    fn global_tool_switch_remains_authoritative() {
        let settings = ToolSettings {
            enabled: false,
            ..ToolSettings::default()
        }
        .for_chat_web_enabled(true);

        assert!(!settings.any_tool_enabled());
        assert!(!settings.web_tools_enabled());
    }
}
