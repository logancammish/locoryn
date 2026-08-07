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
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            web_search: true,
            fetch_webpage: true,
            conversation_search: true,
        }
    }
}

impl ToolSettings {
    pub fn any_tool_enabled(&self) -> bool {
        self.enabled && (self.web_search || self.fetch_webpage || self.conversation_search)
    }

    pub fn web_tools_enabled(&self) -> bool {
        self.enabled && (self.web_search || self.fetch_webpage)
    }
}
