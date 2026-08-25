use std::{
    collections::HashMap,
    fmt,
    sync::{Arc, Mutex},
};

use crate::{
    GUIState, Program,
    inference::{BackendConnections, GenerationDetails, InferenceBackend},
    tools::web_search::WebSource,
};
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub enum Correspondence {
    Bot {
        text: String,
        model: Option<String>,
        thinking_seconds: Option<u64>,
        /// Generation speed for this reply. This uses backend timing when it is
        /// available and otherwise times the generated stream locally.
        tokens_per_second: Option<f32>,
        /// Counts and timings that explain how the displayed token rate was
        /// obtained. Missing for chats saved by older Locoryn versions.
        generation_details: Option<GenerationDetails>,
        sources: Vec<WebSource>,
        web_search_used: bool,
    },
    User {
        text: String,
        images: Vec<ChatImage>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "role", content = "text", rename_all = "lowercase")]
pub enum StoredMessage {
    User(String),
    Bot(String),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SavedChat {
    pub id: String,
    pub title: String,
    pub updated_at: String,
    #[serde(default)]
    pub pinned: bool,
    pub context: Vec<String>,
    pub messages: Vec<StoredMessage>,
    /// Metadata is kept separately so older `role`/`text` chat files remain readable.
    #[serde(default)]
    pub models: Vec<Option<String>>,
    #[serde(default)]
    pub thinking_seconds: Vec<Option<u64>>,
    /// Generation speed in tokens per second for each reply. `None` when the
    /// response gave no token statistics (including cancellations and older
    /// chats).
    #[serde(default)]
    pub tokens_per_second: Vec<Option<f32>>,
    #[serde(default)]
    pub generation_details: Vec<Option<GenerationDetails>>,
    #[serde(default)]
    pub sources: Vec<Vec<WebSource>>,
    #[serde(default)]
    pub web_search_used: Vec<bool>,
    /// `None` lets chats saved before 0.5.2 inherit the global default.
    #[serde(default)]
    pub web_search_enabled: Option<bool>,
    /// The profile that owns this chat. `None` marks chats saved before
    /// profiles existed; they belong to the legacy "Outdated Profile".
    /// Older app versions ignore this field and keep reading every chat.
    #[serde(default)]
    pub profile: Option<String>,
}

impl SavedChat {
    pub fn from_current(
        id: String,
        title: String,
        chat: &CurrentChat,
        web_search_enabled: bool,
        profile: String,
    ) -> Self {
        Self {
            id,
            title,
            updated_at: Local::now().to_rfc3339(),
            pinned: false,
            context: chat.chats.clone(),
            messages: chat
                .messages
                .iter()
                .map(|message| match message {
                    Correspondence::User { text, .. } => StoredMessage::User(text.clone()),
                    Correspondence::Bot { text, .. } => StoredMessage::Bot(text.clone()),
                })
                .collect(),
            models: chat
                .messages
                .iter()
                .map(|message| match message {
                    Correspondence::Bot { model, .. } => model.clone(),
                    Correspondence::User { .. } => None,
                })
                .collect(),
            thinking_seconds: chat
                .messages
                .iter()
                .map(|message| match message {
                    Correspondence::Bot {
                        thinking_seconds, ..
                    } => *thinking_seconds,
                    Correspondence::User { .. } => None,
                })
                .collect(),
            tokens_per_second: chat
                .messages
                .iter()
                .map(|message| match message {
                    Correspondence::Bot {
                        tokens_per_second, ..
                    } => *tokens_per_second,
                    Correspondence::User { .. } => None,
                })
                .collect(),
            generation_details: chat
                .messages
                .iter()
                .map(|message| match message {
                    Correspondence::Bot {
                        generation_details, ..
                    } => *generation_details,
                    Correspondence::User { .. } => None,
                })
                .collect(),
            sources: chat
                .messages
                .iter()
                .map(|message| match message {
                    Correspondence::Bot { sources, .. } => sources.clone(),
                    Correspondence::User { .. } => Vec::new(),
                })
                .collect(),
            web_search_used: chat
                .messages
                .iter()
                .map(|message| {
                    matches!(
                        message,
                        Correspondence::Bot {
                            web_search_used: true,
                            ..
                        }
                    )
                })
                .collect(),
            web_search_enabled: Some(web_search_enabled),
            profile: Some(profile),
        }
    }

    pub fn to_current(&self) -> CurrentChat {
        CurrentChat {
            chats: self.context.clone(),
            messages: self
                .messages
                .iter()
                .enumerate()
                .map(|(index, message)| match message {
                    StoredMessage::User(text) => Correspondence::User {
                        text: text.clone(),
                        images: Vec::new(),
                    },
                    StoredMessage::Bot(text) => Correspondence::Bot {
                        text: text.clone(),
                        model: self.models.get(index).cloned().flatten(),
                        thinking_seconds: self.thinking_seconds.get(index).copied().flatten(),
                        tokens_per_second: self.tokens_per_second.get(index).copied().flatten(),
                        generation_details: self.generation_details.get(index).copied().flatten(),
                        sources: self.sources.get(index).cloned().unwrap_or_default(),
                        web_search_used: self.web_search_used.get(index).copied().unwrap_or(false),
                    },
                })
                .collect(),
            bot_responding: false,
        }
    }
}

#[cfg(test)]
mod saved_chat_tests {
    use super::{Correspondence, CurrentChat, SavedChat};

    #[test]
    fn old_saved_chats_default_to_unpinned() {
        let json = r#"{
            "id":"chat-1",
            "title":"Older chat",
            "updated_at":"2026-01-01T00:00:00Z",
            "context":[],
            "messages":[]
        }"#;

        let chat: SavedChat = serde_json::from_str(json).unwrap();
        assert!(!chat.pinned);
        assert!(chat.models.is_empty());
        assert!(chat.thinking_seconds.is_empty());
        assert!(chat.tokens_per_second.is_empty());
        assert!(chat.generation_details.is_empty());
        assert!(chat.sources.is_empty());
        assert!(chat.web_search_used.is_empty());
        assert_eq!(chat.web_search_enabled, None);
        assert_eq!(chat.profile, None);
    }

    #[test]
    fn response_metadata_survives_saved_chat_round_trip() {
        let current = CurrentChat {
            chats: vec![],
            messages: vec![
                Correspondence::User {
                    text: "Question".into(),
                    images: Vec::new(),
                },
                Correspondence::Bot {
                    text: "<think>Work</think>Answer".into(),
                    model: Some("model-a".into()),
                    thinking_seconds: Some(30),
                    tokens_per_second: Some(18.75),
                    generation_details: Some(crate::inference::GenerationDetails {
                        prompt_tokens: Some(42),
                        output_tokens: Some(75),
                        generation_duration_ms: Some(4_000),
                        response_duration_ms: Some(4_500),
                        ..crate::inference::GenerationDetails::default()
                    }),
                    sources: vec![crate::tools::web_search::WebSource {
                        title: "Example".into(),
                        url: "https://example.com".into(),
                    }],
                    web_search_used: true,
                },
            ],
            bot_responding: false,
        };

        let saved = SavedChat::from_current(
            "chat-1".into(),
            "Question".into(),
            &current,
            true,
            "profile-1".into(),
        );
        assert_eq!(saved.web_search_enabled, Some(true));
        assert_eq!(saved.profile.as_deref(), Some("profile-1"));
        assert_eq!(saved.tokens_per_second, vec![None, Some(18.75)]);
        assert_eq!(saved.generation_details[1].unwrap().output_tokens, Some(75));
        let reopened = saved.to_current();
        assert!(matches!(
            &reopened.messages[1],
            Correspondence::Bot {
                model: Some(model),
                thinking_seconds: Some(30),
                tokens_per_second: Some(tps),
                generation_details: Some(details),
                sources,
                web_search_used: true,
                ..
            } if model == "model-a"
                && sources.len() == 1
                && details.prompt_tokens == Some(42)
                && (tps - 18.75).abs() < f32::EPSILON
        ));
    }
}

/// The profile that owns every chat saved before profiles existed.
pub const LEGACY_PROFILE_ID: &str = "legacy";
pub const LEGACY_PROFILE_NAME: &str = "Outdated Profile";

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    /// Display label shown in the profile switcher. It is private to the
    /// user and never sent to the model.
    pub name: String,
    /// The name the model is told the user goes by. Empty shares no name.
    #[serde(default)]
    pub user_name: String,
    /// Per-profile text appended to the system prompt after the global
    /// custom instructions.
    #[serde(default)]
    pub custom_instructions: String,
}

/// Persisted beside the other app data. Kept tolerant of missing fields so a
/// partially written file still yields a usable legacy profile.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ProfileRegistry {
    pub profiles: Vec<Profile>,
    pub active_profile_id: String,
}

#[derive(Clone, Debug)]
pub struct ChatImage {
    pub name: String,
    pub mime_type: String,
    pub bytes: Vec<u8>,
    /// Keep one stable renderer handle for the lifetime of the attachment.
    /// Recreating a handle in every `view` assigns a new cache id each frame,
    /// which can make previews flash and then disappear.
    pub preview_handle: iced::widget::image::Handle,
}

#[derive(Clone)]
pub struct DebugMessage {
    pub message: String,
    pub is_error: bool,
}

#[derive(Clone, Debug)]
pub struct CurrentChat {
    pub chats: Vec<String>,
    pub messages: Vec<Correspondence>,
    pub bot_responding: bool,
}
impl CurrentChat {
    fn push_chat(&mut self, chat: String) {
        self.chats.push(chat);
    }
    fn generate_new_message(user_message: String, bot_response: String) -> String {
        format!(
            "User: {}\nAI Language Model: {}",
            user_message, bot_response
        )
    }
    pub fn generate_and_push(&mut self, user_message: String, bot_response: String) {
        let new_message = Self::generate_new_message(user_message, bot_response);
        self.push_chat(new_message);
    }
    pub fn unravel(&self) -> String {
        self.chats.join("\n")
    }

    pub fn push_message(&mut self, correspondence: Correspondence) {
        self.messages.push(correspondence);
    }
}

// AppState keeps information on certain important information
pub struct AppState {
    pub filtering: bool,
    pub interface_theme: InterfaceTheme,
    pub backend_state: Arc<Mutex<String>>,
    pub bots_list: Arc<Mutex<Vec<String>>>,
    pub gui_state: GUIState,
}

// SystemPrompt saves the current system prompts and the currently selected system prompt
#[derive(Clone)]
pub struct SystemPrompt {
    pub system_prompts_as_hashmap: HashMap<String, String>,
    pub system_prompts_as_vec: Arc<Mutex<Vec<String>>>,
    pub system_prompt: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct DynamicPromptSettings {
    pub include_date: bool,
    pub include_time: bool,
    pub custom_instructions: String,
}

impl Default for DynamicPromptSettings {
    fn default() -> Self {
        Self {
            include_date: true,
            include_time: true,
            custom_instructions: String::new(),
        }
    }
}

impl DynamicPromptSettings {
    /// The user's name and extra instructions come from the active profile.
    /// The profile's display name stays private and is never included.
    pub fn apply(
        &self,
        base_prompt: &str,
        now: chrono::DateTime<Local>,
        user_name: &str,
        profile_instructions: &str,
    ) -> String {
        let mut prompt = base_prompt.trim().to_string();
        let mut dynamic = Vec::new();

        if self.include_date {
            dynamic.push(format!("Current date: {}", now.format("%Y-%m-%d")));
        }
        if self.include_time {
            dynamic.push(format!(
                "Current local time: {} {}",
                now.format("%H:%M:%S"),
                now.format("%:z")
            ));
        }
        let user_name = user_name.trim();
        if !user_name.is_empty() {
            // A bare fact is easy for smaller models to overlook, so phrase
            // the name as an instruction the model is expected to act on.
            dynamic.push(format!(
                "The user's name is {user_name}. Use this name when addressing the user."
            ));
        }
        if !self.custom_instructions.trim().is_empty() {
            dynamic.push(self.custom_instructions.trim().to_string());
        }
        if !profile_instructions.trim().is_empty() {
            dynamic.push(profile_instructions.trim().to_string());
        }

        if !dynamic.is_empty() {
            if !prompt.is_empty() {
                prompt.push_str("\n\n");
            }
            prompt.push_str("Dynamic context:\n");
            prompt.push_str(&dynamic.join("\n"));
        }
        prompt
    }
}

impl SystemPrompt {
    // gets the currently selected system prompt
    pub fn get_current(program: &Program) -> Option<String> {
        let system_prompt: SystemPrompt = program.system_prompt.clone();
        let system_prompt_as_string: String = match system_prompt.system_prompt {
            Some(system_prompt) => system_prompt,
            None => {
                println!("Error getting system prompt");
                Channels::send_request_to_channel(
                    Arc::clone(&program.channels.debug_channel),
                    DebugMessage {
                        message: "Could not get system prompt, is it selected?".to_string(),
                        is_error: true,
                    },
                );
                return None;
            }
        };

        if system_prompt
            .system_prompts_as_hashmap
            .contains_key(&system_prompt_as_string)
        {
            system_prompt
                .system_prompts_as_hashmap
                .get(&system_prompt_as_string)
                .cloned()
        } else {
            println!("system prompt is None");
            Channels::send_request_to_channel(
                Arc::clone(&program.channels.debug_channel),
                DebugMessage {
                    message: "Could not get system prompt, is it selected?".to_string(),
                    is_error: true,
                },
            );
            None
        }
    }
}

// UserInformation saves certain important information about the program specific to the current user
#[derive(Clone)]
pub struct UserInformation {
    pub backend: InferenceBackend,
    pub backend_connections: BackendConnections,
    pub model: Option<String>,
    pub thinking_level: ThinkingLevel,
    /// The exact controls accepted by the selected model. A regular thinking
    /// model generally exposes Off/On, while effort-aware models expose their
    /// reported named levels.
    pub thinking_levels: Vec<ThinkingLevel>,
    /// `None` means capability detection is still in flight or unavailable.
    pub thinking_supported: Option<bool>,
    pub vision_supported: Option<bool>,
    pub max_response_tokens: u32,
    pub context_tokens: u32,
    pub temperature: f32,
    pub text_size: f32,
    pub font_family: FontFamily,
    pub chat_history: Arc<Mutex<CurrentChat>>,
    pub current_chat_history_enabled: bool,
    pub language: Language,
}

impl UserInformation {
    pub fn active_connection(&self) -> &crate::inference::HostLocation {
        self.backend_connections.active(self.backend)
    }

    pub fn active_connection_mut(&mut self) -> &mut crate::inference::HostLocation {
        self.backend_connections.active_mut(self.backend)
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FontFamily {
    #[default]
    #[serde(alias = "sansserif")]
    SansSerif,
    Serif,
    Monospace,
}

impl FontFamily {
    pub const ALL: [Self; 3] = [Self::SansSerif, Self::Serif, Self::Monospace];
}

impl fmt::Display for FontFamily {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::SansSerif => "Sans-serif",
            Self::Serif => "Serif",
            Self::Monospace => "Monospace",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceTheme {
    #[default]
    Dark,
    Light,
    Modern,
}

impl InterfaceTheme {
    pub const ALL: [Self; 3] = [Self::Dark, Self::Light, Self::Modern];
}

impl fmt::Display for InterfaceTheme {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Modern => "Modern",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    English,
    Spanish,
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::English => "English",
            Self::Spanish => "Español (experimental)",
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThinkingLevel {
    #[default]
    Off,
    On,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
}

impl ThinkingLevel {
    pub const ORDERED: [Self; 8] = [
        Self::Off,
        Self::On,
        Self::Minimal,
        Self::Low,
        Self::Medium,
        Self::High,
        Self::XHigh,
        Self::Max,
    ];

    pub fn from_api_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "false" | "none" | "off" | "disabled" => Some(Self::Off),
            "true" | "on" | "enabled" => Some(Self::On),
            "minimal" | "min" => Some(Self::Minimal),
            "low" => Some(Self::Low),
            "medium" | "med" => Some(Self::Medium),
            "high" => Some(Self::High),
            "xhigh" | "extra_high" | "extra-high" => Some(Self::XHigh),
            "max" | "maximum" => Some(Self::Max),
            _ => None,
        }
    }

    pub fn api_value(self) -> serde_json::Value {
        match self {
            Self::Off => serde_json::Value::Bool(false),
            Self::On => serde_json::Value::Bool(true),
            Self::Minimal => serde_json::Value::String("minimal".into()),
            Self::Low => serde_json::Value::String("low".into()),
            Self::Medium => serde_json::Value::String("medium".into()),
            Self::High => serde_json::Value::String("high".into()),
            Self::XHigh => serde_json::Value::String("xhigh".into()),
            Self::Max => serde_json::Value::String("max".into()),
        }
    }
}

impl fmt::Display for ThinkingLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Off => "Off",
            Self::On => "On",
            Self::Minimal => "Minimal",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::XHigh => "Extra high",
            Self::Max => "Maximum",
        })
    }
}

/// Channels
/// These mpsc channels provide communication between runtimes.
/// debug_channel: mpsc channel for sending debug information to GUI

#[derive(Clone)]
pub struct Channels {
    pub debug_channel: Arc<
        Mutex<(
            std::sync::mpsc::Sender<DebugMessage>,
            std::sync::mpsc::Receiver<DebugMessage>,
        )>,
    >,
}

impl Channels {
    pub fn send_request_to_channel<T: Send + Clone>(
        channel: Arc<Mutex<(std::sync::mpsc::Sender<T>, std::sync::mpsc::Receiver<T>)>>,
        message: T,
    ) {
        match channel.lock() {
            Ok(channel) => {
                if let Err(e) = channel.0.send(message) {
                    eprintln!("Failed to send: {}", e);
                }
            }
            Err(e) => {
                eprintln!("Failed to send: {}", e);
            }
        }
    }
}
// Prompt stores the current composer text.
pub struct Prompt {
    pub prompt: String,
    pub editor: iced::widget::text_editor::Content,
}

#[cfg(test)]
mod dynamic_prompt_tests {
    use super::DynamicPromptSettings;
    use chrono::{FixedOffset, Local, TimeZone};

    #[test]
    fn dynamic_prompt_includes_enabled_fields_only() {
        let settings = DynamicPromptSettings {
            include_date: true,
            include_time: false,
            custom_instructions: "Prefer concise answers.".into(),
        };
        let now = FixedOffset::east_opt(12 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 7, 26, 14, 30, 0)
            .unwrap()
            .with_timezone(&Local);
        let prompt = settings.apply(
            "You are helpful.",
            now,
            "Aroha",
            "This profile is for work tasks.",
        );

        assert!(prompt.contains("Current date: 2026-07-26"));
        assert!(!prompt.contains("Current local time:"));
        assert!(prompt.contains("The user's name is Aroha."));
        assert!(prompt.contains("Prefer concise answers."));
        assert!(prompt.contains("This profile is for work tasks."));
    }

    #[test]
    fn dynamic_prompt_omits_blank_profile_names_and_instructions() {
        let settings = DynamicPromptSettings {
            include_date: false,
            include_time: false,
            custom_instructions: String::new(),
        };
        let now = FixedOffset::east_opt(12 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 7, 26, 14, 30, 0)
            .unwrap()
            .with_timezone(&Local);
        let prompt = settings.apply("You are helpful.", now, "   ", "  ");

        assert!(!prompt.contains("The user's name is"));
        assert!(!prompt.contains("Dynamic context:"));
    }

    #[test]
    fn legacy_dynamic_prompt_settings_still_load_without_user_name() {
        let json = r#"{
            "include_date": false,
            "include_time": false,
            "include_user_name": true,
            "user_name": "Old setting",
            "custom_instructions": ""
        }"#;

        let settings: DynamicPromptSettings = serde_json::from_str(json).unwrap();
        assert!(!settings.include_date);
        assert!(!settings.include_time);
        assert!(settings.custom_instructions.is_empty());
    }
}

#[cfg(test)]
mod profile_tests {
    use super::{LEGACY_PROFILE_ID, Profile, ProfileRegistry};

    #[test]
    fn empty_registry_defaults_to_no_profiles() {
        let registry: ProfileRegistry = serde_json::from_str("{}").unwrap();
        assert!(registry.profiles.is_empty());
        assert!(registry.active_profile_id.is_empty());
    }

    #[test]
    fn registry_round_trips_profiles_and_active_selection() {
        let registry = ProfileRegistry {
            profiles: vec![
                Profile {
                    id: LEGACY_PROFILE_ID.into(),
                    name: "Outdated Profile".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
                Profile {
                    id: "profile-1".into(),
                    name: "Work".into(),
                    user_name: "Logan".into(),
                    custom_instructions: "Keep answers brief.".into(),
                },
            ],
            active_profile_id: "profile-1".into(),
        };

        let json = serde_json::to_string(&registry).unwrap();
        let restored: ProfileRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.profiles, registry.profiles);
        assert_eq!(restored.active_profile_id, "profile-1");
    }

    #[test]
    fn profiles_saved_before_customisation_default_to_empty_extras() {
        let json = r#"{"id":"profile-1","name":"Work"}"#;

        let profile: Profile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.name, "Work");
        assert!(profile.user_name.is_empty());
        assert!(profile.custom_instructions.is_empty());
    }
}
