#![windows_subsystem = "windows"]

use std::cmp::Ordering as ComparisonOrdering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::Local;
use iced::{Element, Point, Size, Subscription, Task, Theme, clipboard, keyboard, mouse, time};
use iced_widget::markdown;
use ollama_rs::Ollama;
use ollama_rs::generation::completion::GenerationResponse;
use ollama_rs::generation::completion::request::GenerationRequest;
use ollama_rs::models::ModelOptions;
use rustrict::{Censor, Type};
mod app;
mod gui;
mod web_search;

use crate::app::{
    AppState, Channels, ChatImage, Correspondence, CurrentChat, DebugMessage,
    DynamicPromptSettings, History, HostLocation, LEGACY_PROFILE_ID, LEGACY_PROFILE_NAME,
    Language, Log, Profile, ProfileRegistry, Prompt, SavedChat, SystemPrompt, ThinkingLevel,
    UserInformation,
};
use crate::web_search::{
    ToolLoopProgress, ToolLoopRequest, WebSearchProviderKind, WebSearchSettings, WebSearchState,
    create_search_provider, run_tool_loop, send_ollama_request_with_retry, validate_public_url,
};

/// Tick points:
/// Each tick occurs every TICK_MS; these constants decide what happens on each tick.
const VERSION_TICK: i32 = 2;
const MAX_TICK: i32 = 59;
const BOT_LIST_TICK: i32 = 3;
const TICK_MS: u64 = 200;
/// Live output and indeterminate progress need a frame-oriented cadence. Keeping
/// this separate from housekeeping avoids making disk/network polling run at
/// animation speed.
const UI_FRAME_MS: u64 = 16;
const LIVE_RENDER_MS: u128 = 16;
const MAX_MARKDOWN_IMAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_IMAGE_RESPONSE_BYTES: usize = MAX_MARKDOWN_IMAGE_BYTES * 2;
const MAX_MARKDOWN_IMAGE_PIXELS: u64 = 32_000_000;
const SETTINGS_SAVE_DEBOUNCE_MS: u64 = 450;
const SETTINGS_FEEDBACK_DURATION_MS: u64 = 420;
const DEFAULT_MAX_RESPONSE_TOKENS: u32 = 32_768;
const DEFAULT_CONTEXT_TOKENS: u32 = 131_072;
const MIN_RESPONSE_TOKENS: u32 = 512;
const MAX_RESPONSE_TOKENS: u32 = 1_048_576;
const MIN_CONTEXT_TOKENS: u32 = 4_096;
const MAX_CONTEXT_TOKENS: u32 = 4_194_304;
const DEFAULT_SIDEBAR_WIDTH: f32 = 278.0;
const MIN_SIDEBAR_WIDTH: f32 = 210.0;
const MAX_SIDEBAR_WIDTH: f32 = 460.0;
const DEFAULT_COMPOSER_HEIGHT: f32 = 142.0;
const MIN_COMPOSER_HEIGHT: f32 = 118.0;
const MAX_COMPOSER_HEIGHT: f32 = 320.0;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(target_os = "linux")]
const LINUX_APP_ID: &str = "io.github.logancammish.locoryn";
const UPDATE_RELEASE_API: &str =
    "https://api.github.com/repos/logancammish/locoryn/releases/latest";
const UPDATE_RELEASE_PAGE: &str = "https://github.com/logancammish/locoryn/releases/latest";

#[derive(PartialEq, Clone, Copy)]
pub enum GUIState {
    InfoPopup,
    Main,
    Settings,
    AdvancedSettings,
    Images,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UiResizeTarget {
    Sidebar,
    Composer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SettingsFeedbackTarget {
    MaxResponse,
    ApplyMaxResponse,
    ContextWindow,
    ApplyContextWindow,
    Temperature,
    TextSize,
    SearchResultLimit,
    MaximumSearches,
    RequiredSearches,
    MaximumPageReads,
    RequiredPageReads,
    ToolRounds,
    RequestTimeout,
    BatchTokens,
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
#[serde(default)]
struct UiLayoutSettings {
    sidebar_width: f32,
    composer_height: f32,
}

impl Default for UiLayoutSettings {
    fn default() -> Self {
        Self {
            sidebar_width: DEFAULT_SIDEBAR_WIDTH,
            composer_height: DEFAULT_COMPOSER_HEIGHT,
        }
    }
}

impl UiLayoutSettings {
    fn normalized(mut self) -> Self {
        self.sidebar_width = self
            .sidebar_width
            .clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
        self.composer_height = self
            .composer_height
            .clamp(MIN_COMPOSER_HEIGHT, MAX_COMPOSER_HEIGHT);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ModelCapabilities {
    thinking_levels: Vec<ThinkingLevel>,
    vision: bool,
    image_generation: bool,
}

#[derive(Debug, Clone)]
enum Message {
    ChangeBatchTokens(i32),
    ToggleFastStreaming,
    ToggleChatMenu,
    ToggleWebSearch,
    ToggleMultipleWebSearches,
    ToggleDeepResearchControls,
    ToggleChatWebSearch,
    WebSearchProviderChange(WebSearchProviderKind),
    WebSearchApiKeyChange(String),
    WebSearchResultLimitChange(f32),
    WebSearchMaximumSearchesChange(f32),
    WebSearchMaximumPageFetchesChange(f32),
    WebSearchMinimumSuccessfulSearchesChange(f32),
    WebSearchMinimumIndependentPagesChange(f32),
    WebSearchToolIterationLimitChange(f32),
    WebSearchRequestTimeoutChange(f32),
    WebSearchCustomInstructionsChange(String),
    OpenSource(String),
    CheckForUpdates,
    AppUpdateChecked(Result<AppUpdateInfo, String>),
    OpenAppUpdate(String),
    UrlOpened(Result<(), String>),
    NewChat,
    OpenChat(String),
    ToggleChatPin(String),
    DeleteChat(String),
    DeleteTemporaryChat(String),
    ToggleTemporaryChat,
    ChooseChatFolder,
    ChatFolderSelected(Option<PathBuf>),
    AsyncResult(()),
    PromptFinished(String),
    ListPrompt,
    ThinkingLevelChange(ThinkingLevel),
    ToggleImages,
    PickImage,
    DropImage(PathBuf),
    PasteImage,
    ImageLoaded(Result<ChatImage, String>),
    ImagesLoaded(Result<Vec<ChatImage>, String>),
    MarkdownImageLoaded {
        url: String,
        result: Result<iced::widget::image::Handle, String>,
    },
    RemoveImage(usize),
    GenerateImage,
    ImageGenerated(Result<String, String>),
    CopyImage(String),
    ModelCapabilitiesKnown(String, Option<ModelCapabilities>),
    ToggleSettings,
    SystemPromptChange(String),
    Prompt(String),
    StopResponse,
    EditPrompt(iced::widget::text_editor::Action),
    UseSuggestion(String),
    None,
    KeyPressed(keyboard::Key, keyboard::Modifiers),
    KeyReleased(keyboard::Key),
    StartUiResize(UiResizeTarget),
    UiResizeMoved(Point),
    StopUiResize,
    WindowResized(Size),
    FrameTick,
    Tick,
    CopyPressed(String),
    CopyLatestResponse,
    ToggleThinking(usize),
    ToggleSources(usize),
    UpdateTextSize(f32),
    InstallationPrompt,
    ModelChange(String),
    InstallModel(String),
    UpdateInstall(String),
    UpdateTemperature(f32),
    UpdateMaxResponseTokens(f32),
    EditMaxResponseTokens(String),
    ApplyMaxResponseTokens,
    UpdateContextTokens(f32),
    EditContextTokens(String),
    ApplyContextTokens,
    ToggleCodeChecking,
    CheckCode(String, String),
    CodeChecked(Result<String, String>),
    ToggleDynamicDate,
    ToggleDynamicTime,
    DynamicCustomInstructionsChanged(String),
    ToggleProfileMenu,
    SelectProfile(String),
    ProfileNameInputChanged(String),
    CreateProfile,
    StartEditProfile(String),
    CancelEditProfile,
    ProfileEditNameChanged(String),
    ProfileEditUserNameChanged(String),
    ProfileEditInstructionsChanged(String),
    ConfirmProfileEdits,
    DeleteProfile(String),
    LanguageChange(Language),
    ToggleInfoPopup,
    ToggleChatHistory,
    ToggleFiltering,
    ToggleDarkMode,
    WipeChatHistory,
    ToggleAdvancedSettings,
    ChangeIp(String),
    ChangePort(String),
}

struct ActivePrompt {
    chat_history: Arc<Mutex<CurrentChat>>,
    response_text: String,
    thinking_text: String,
    parsed_markdown: Vec<markdown::Item>,
    render_receiver: crossbeam_channel::Receiver<LiveRender>,
    web_search_state: WebSearchState,
    web_search_state_receiver: crossbeam_channel::Receiver<WebSearchState>,
    web_progress_receiver: tokio::sync::watch::Receiver<ToolLoopProgress>,
    cancel: Arc<AtomicBool>,
    model_name: String,
    started_at: Instant,
    response_start_index: usize,
    had_image: bool,
    web_search_enabled: bool,
    temporary: bool,
    /// The profile that owns the chat, captured when the prompt started so a
    /// profile switch mid-response cannot orphan the finished chat.
    profile_id: String,
}

struct LiveRender {
    text: String,
    thinking: String,
    markdown: Vec<markdown::Item>,
}

struct TemporaryChatSession {
    chat_history: Arc<Mutex<CurrentChat>>,
    web_search_enabled: bool,
    profile_id: String,
}

#[derive(Clone, Debug)]
struct AppUpdateInfo {
    latest_version: String,
    release_url: String,
}

#[derive(Clone, Debug, Default)]
enum AppUpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate {
        latest_version: String,
    },
    Available {
        latest_version: String,
        release_url: String,
    },
    Failed(String),
}

struct VisionResponse {
    markdown: Vec<markdown::Item>,
}

#[derive(Clone)]
enum MarkdownImageState {
    Loading,
    Ready(iced::widget::image::Handle),
    Failed(String),
}

struct Program {
    active_prompts: HashMap<String, ActivePrompt>,
    temporary_chats: HashMap<String, TemporaryChatSession>,
    chat_notices: HashMap<String, (DebugMessage, Instant)>,
    chat_notice_sender: crossbeam_channel::Sender<(String, DebugMessage)>,
    chat_notice_receiver: crossbeam_channel::Receiver<(String, DebugMessage)>,
    current_tick: i32,
    installing_model: String,
    app_update_state: AppUpdateState,

    debug_message: DebugMessage,
    debug_message_set_at: Option<Instant>,

    /// Parsed markdown cache for finished chat messages.
    /// This is needed because markdown::view borrows parsed markdown items.
    chat_markdown_cache: Vec<Vec<markdown::Item>>,

    /// UI-owned snapshot of the open chat. Background workers write through the
    /// shared history, but ordinary view rebuilds should not clone the complete
    /// conversation on every keystroke or animation frame.
    chat_messages_cache: Vec<Correspondence>,
    chat_thinking_cache: Vec<String>,
    chat_visible_text_cache: Vec<String>,

    /// One model label per chat message.
    /// User messages use None. Bot messages store the model that generated them.
    chat_model_name_cache: Vec<Option<String>>,

    /// Used for brief copy feedback animations/buttons.
    last_copied_text: Option<String>,
    last_copied_at: Option<Instant>,

    pending_images: Vec<ChatImage>,
    generated_images: Vec<String>,
    is_generating_image: bool,
    vision_responses: HashMap<String, VisionResponse>,
    markdown_images: HashMap<String, MarkdownImageState>,

    /// Message indexes whose reasoning disclosure is open. Reasoning is hidden by default.
    expanded_thinking: HashSet<usize>,
    /// Message indexes whose source disclosure is open. Sources are collapsed by default.
    expanded_sources: HashSet<usize>,
    deep_research_controls_open: bool,
    settings_feedback: Option<(SettingsFeedbackTarget, Instant)>,
    brand_icon: iced::widget::image::Handle,

    system_prompt: SystemPrompt,
    app_state: AppState,
    channels: Channels,
    user_information: UserInformation,
    prompt: Prompt,
    batch_tokens: i32,
    fast_streaming: bool,
    chat_menu_open: bool,
    /// Normalized sidebar reveal progress. This is animated instead of
    /// switching between two hard-coded widths in a single frame.
    sidebar_animation: f32,
    /// A lightweight phase used by live activity affordances.
    ui_motion: f32,
    /// Drives a short, eased reveal when the user changes pages or chats.
    page_reveal: f32,
    ui_layout: UiLayoutSettings,
    ui_resize_target: Option<UiResizeTarget>,
    window_size: Size,
    temporary_chat: bool,
    web_search_settings: WebSearchSettings,
    web_search_for_chat: bool,
    current_chat_id: String,
    open_chat_dirty: bool,
    saved_chats: Vec<SavedChat>,
    chat_storage_dir: PathBuf,
    /// Every chat belongs to a profile. The legacy profile collects the chats
    /// saved before profiles existed.
    profiles: Vec<Profile>,
    active_profile_id: String,
    profile_menu_open: bool,
    profile_name_input: String,
    editing_profile_id: Option<String>,
    profile_edit_name: String,
    profile_edit_user_name: String,
    profile_edit_instructions: String,
    code_checking_enabled: bool,
    dynamic_prompt_settings: DynamicPromptSettings,
    max_response_tokens_input: String,
    context_tokens_input: String,
    pending_settings: serde_json::Map<String, serde_json::Value>,
    settings_dirty_at: Option<Instant>,
}

fn default_chat_storage_dir() -> PathBuf {
    app_data_dir().join("chats")
}

#[cfg(test)]
fn app_data_dir() -> PathBuf {
    std::env::temp_dir().join(format!("locoryn-test-data-{}", std::process::id()))
}

#[cfg(not(test))]
fn app_data_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        let base = PathBuf::from(base);
        let preferred = base.join("Locoryn");
        let legacy = base.join("Ollama GUI");
        return if preferred.exists() || !legacy.exists() {
            preferred
        } else {
            legacy
        };
    }
    #[cfg(target_os = "macos")]
    if let Some(home) = std::env::var_os("HOME") {
        let base = PathBuf::from(home).join("Library/Application Support");
        let preferred = base.join("Locoryn");
        let legacy = base.join("Ollama GUI");
        return if preferred.exists() || !legacy.exists() {
            preferred
        } else {
            legacy
        };
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(base) = std::env::var_os("XDG_DATA_HOME") {
            let base = PathBuf::from(base);
            let preferred = base.join("locoryn");
            let legacy = base.join("ollama-gui");
            return if preferred.exists() || !legacy.exists() {
                preferred
            } else {
                legacy
            };
        }
        if let Some(home) = std::env::var_os("HOME") {
            let base = PathBuf::from(home).join(".local/share");
            let preferred = base.join("locoryn");
            let legacy = base.join("ollama-gui");
            return if preferred.exists() || !legacy.exists() {
                preferred
            } else {
                legacy
            };
        }
    }
    PathBuf::from("output")
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut path = path.as_os_str().to_os_string();
    path.push(suffix);
    PathBuf::from(path)
}

/// Replace a persisted file from a fully written sibling and retain the
/// previous valid copy. On Windows, where rename cannot replace an existing
/// file, the backup also closes the small remove/rename crash window.
fn write_file_safely(path: &Path, contents: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let temporary = sidecar_path(path, ".tmp");
    let backup = sidecar_path(path, ".bak");
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(contents)?;
    file.sync_all()?;
    drop(file);

    if path.exists() {
        fs::copy(path, &backup)?;
    }

    #[cfg(target_os = "windows")]
    if path.exists() {
        fs::remove_file(path)?;
    }

    if let Err(error) = fs::rename(&temporary, path) {
        if !path.exists() && backup.exists() {
            let _ = fs::copy(&backup, path);
        }
        return Err(error);
    }

    Ok(())
}

fn write_json_safely<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let contents = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_file_safely(path, &contents).map_err(|error| error.to_string())
}

fn read_json_with_backup<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    [path.to_path_buf(), sidecar_path(path, ".bak")]
        .into_iter()
        .find_map(|candidate| {
            fs::read_to_string(candidate)
                .ok()
                .and_then(|contents| serde_json::from_str(&contents).ok())
        })
}

fn chat_location_settings_path() -> PathBuf {
    app_data_dir().join("chat-location.json")
}

fn user_settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

fn history_path() -> PathBuf {
    app_data_dir().join("history.json")
}

fn profiles_path() -> PathBuf {
    app_data_dir().join("profiles.json")
}

/// Chats saved before profiles existed carry no profile id; they belong to
/// the legacy "Outdated Profile".
fn chat_profile_id(chat: &SavedChat) -> &str {
    chat.profile.as_deref().unwrap_or(LEGACY_PROFILE_ID)
}

/// Guarantees the registry contains the legacy profile and points at an
/// existing active profile. Returns whether anything was changed.
fn ensure_legacy_profile(registry: &mut ProfileRegistry) -> bool {
    let mut changed = false;
    if !registry.profiles.iter().any(|profile| profile.id == LEGACY_PROFILE_ID) {
        registry.profiles.insert(
            0,
            Profile {
                id: LEGACY_PROFILE_ID.to_string(),
                name: LEGACY_PROFILE_NAME.to_string(),
                user_name: String::new(),
                custom_instructions: String::new(),
            },
        );
        changed = true;
    }
    let active_is_known = registry
        .profiles
        .iter()
        .any(|profile| profile.id == registry.active_profile_id);
    if !active_is_known {
        registry.active_profile_id = LEGACY_PROFILE_ID.to_string();
        changed = true;
    }
    changed
}

/// Assigns every pre-profile chat to the legacy profile. Returns whether
/// anything was changed.
fn assign_legacy_profile_ids(saved_chats: &mut [SavedChat]) -> bool {
    let mut changed = false;
    for chat in saved_chats.iter_mut() {
        if chat.profile.is_none() {
            chat.profile = Some(LEGACY_PROFILE_ID.to_string());
            changed = true;
        }
    }
    changed
}

/// Frames the ongoing conversation as a single prompt. Naming the person the
/// model is talking to keeps the profile's user name tied to the transcript
/// itself, not only to a line buried in the system prompt.
fn conversation_context_prompt(context: &str, user_name: &str, prompt: &str) -> String {
    let user_name = user_name.trim();
    let partner = if user_name.is_empty() { "a User" } else { user_name };
    format!(
        "The following is a conversation between an AI language model and {partner}. You are the AI language model:
    {context}
    [END CONVERSATION CONTEXT]
    Now, the user is sending another message: {prompt}
    Respond:
    "
    )
}

fn generated_images_dir() -> PathBuf {
    app_data_dir().join("generated")
}

fn load_generated_images() -> Vec<String> {
    let mut images = fs::read_dir(generated_images_dir())
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .map(str::to_ascii_lowercase)
                    .as_deref(),
                Some("png" | "jpg" | "jpeg" | "webp")
            )
        })
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    images.sort();
    images
}

fn collect_reported_thinking_levels(value: &serde_json::Value, levels: &mut Vec<ThinkingLevel>) {
    const LEVEL_KEYS: [&str; 6] = [
        "thinking_levels",
        "think_levels",
        "reasoning_levels",
        "supported_thinking_levels",
        "supported_reasoning_levels",
        "reasoning_efforts",
    ];

    match value {
        serde_json::Value::Object(object) => {
            for (key, child) in object {
                if LEVEL_KEYS.contains(&key.to_ascii_lowercase().as_str()) {
                    let reported = match child {
                        serde_json::Value::Array(values) => values.as_slice(),
                        _ => std::slice::from_ref(child),
                    };
                    for level in reported {
                        if let Some(level) = level.as_str().and_then(ThinkingLevel::from_api_name)
                            && !levels.contains(&level)
                        {
                            levels.push(level);
                        }
                    }
                } else {
                    collect_reported_thinking_levels(child, levels);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                collect_reported_thinking_levels(child, levels);
            }
        }
        _ => {}
    }
}

fn sorted_thinking_levels(mut levels: Vec<ThinkingLevel>) -> Vec<ThinkingLevel> {
    levels.sort_by_key(|level| {
        ThinkingLevel::ORDERED
            .iter()
            .position(|candidate| candidate == level)
            .unwrap_or(usize::MAX)
    });
    levels.dedup();
    levels
}

fn model_capabilities(json: &serde_json::Value) -> Option<ModelCapabilities> {
    let capabilities = json.get("capabilities")?.as_array()?;
    let has = |name| {
        capabilities
            .iter()
            .any(|capability| capability.as_str() == Some(name))
    };
    let thinking = has("thinking");
    let mut thinking_levels = Vec::new();
    if thinking {
        collect_reported_thinking_levels(json, &mut thinking_levels);

        let renderer = json
            .get("renderer")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let parser = json
            .get("parser")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let family = json
            .pointer("/details/family")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let harmony = [renderer, parser, family].iter().any(|value| {
            value.to_ascii_lowercase().contains("harmony")
                || value.to_ascii_lowercase().contains("gptoss")
                || value.to_ascii_lowercase().contains("gpt-oss")
        });

        if thinking_levels.is_empty() && harmony {
            thinking_levels.extend([
                ThinkingLevel::Low,
                ThinkingLevel::Medium,
                ThinkingLevel::High,
            ]);
        }

        let template = json
            .get("template")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if thinking_levels.is_empty() && template.contains("ThinkLevel") {
            let template = template.to_ascii_lowercase();
            for level in ThinkingLevel::ORDERED {
                let api_name = match level {
                    ThinkingLevel::Off => "off",
                    ThinkingLevel::On => "on",
                    ThinkingLevel::Minimal => "minimal",
                    ThinkingLevel::Low => "low",
                    ThinkingLevel::Medium => "medium",
                    ThinkingLevel::High => "high",
                    ThinkingLevel::XHigh => "xhigh",
                    ThinkingLevel::Max => "max",
                };
                if template.contains(&format!("\"{api_name}\""))
                    || template.contains(&format!("'{api_name}'"))
                {
                    thinking_levels.push(level);
                }
            }
        }

        if thinking_levels.is_empty() {
            thinking_levels.extend([ThinkingLevel::Off, ThinkingLevel::On]);
        }
    } else {
        thinking_levels.push(ThinkingLevel::Off);
    }

    Some(ModelCapabilities {
        thinking_levels: sorted_thinking_levels(thinking_levels),
        vision: has("vision"),
        image_generation: has("image"),
    })
}

fn generated_image_payload(body: &serde_json::Value) -> Option<&str> {
    body.get("data")
        .and_then(serde_json::Value::as_array)
        .and_then(|images| images.first())
        .and_then(|image| image.get("b64_json"))
        .and_then(serde_json::Value::as_str)
        // Keep compatibility with early experimental Ollama builds.
        .or_else(|| body.get("image").and_then(serde_json::Value::as_str))
}

async fn generate_image_via_ollama(
    host: String,
    model: String,
    prompt: String,
) -> Result<String, String> {
    let response = reqwest::Client::new()
        .post(format!("{host}/v1/images/generations"))
        .json(&serde_json::json!({
            "model": model,
            "prompt": prompt,
            "size": "1024x1024",
            "response_format": "b64_json"
        }))
        .send()
        .await
        .map_err(|error| format!("Could not reach Ollama: {error}"))?;

    let status = response.status();
    if !status.is_success() {
        let detail = read_response_limited(response, 64 * 1024)
            .await
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default();
        return Err(format!(
            "Ollama image generation failed ({status}): {}",
            detail.trim()
        ));
    }

    let response_bytes = read_response_limited(response, MAX_IMAGE_RESPONSE_BYTES)
        .await
        .map_err(|error| format!("Could not read Ollama's image response: {error}"))?;
    let response_text = String::from_utf8(response_bytes)
        .map_err(|_| "Ollama returned a non-text image response.".to_string())?;
    let body = serde_json::from_str::<serde_json::Value>(&response_text)
        .ok()
        .or_else(|| {
            response_text
                .lines()
                .rev()
                .filter(|line| !line.trim().is_empty())
                .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        })
        .ok_or_else(|| "Ollama returned an invalid image response.".to_string())?;
    let encoded = generated_image_payload(&body).ok_or_else(|| {
            "The selected model returned no image data. Update Ollama and choose a model with the `image` capability.".to_string()
        })?;
    let encoded = encoded.rsplit_once(',').map_or(encoded, |(_, data)| data);
    let bytes = BASE64
        .decode(encoded)
        .map_err(|error| format!("Could not decode Ollama's generated image: {error}"))?;
    validate_image_dimensions(&bytes)?;
    let decoded = image::load_from_memory(&bytes)
        .map_err(|error| format!("Ollama returned unsupported image data: {error}"))?;

    let output_directory = generated_images_dir();
    fs::create_dir_all(&output_directory)
        .map_err(|error| format!("Could not create image output folder: {error}"))?;
    let output_path = output_directory.join(format!(
        "locoryn-image-{}.png",
        chrono::Utc::now().timestamp_millis()
    ));
    decoded
        .save(&output_path)
        .map_err(|error| format!("Could not save generated image: {error}"))?;
    Ok(output_path.to_string_lossy().to_string())
}

/// Installed assets are read-only. Resolve them beside the executable so
/// shortcuts and command-line launches work regardless of their working folder.
fn resource_path(relative: &str) -> PathBuf {
    if let Ok(executable) = std::env::current_exe()
        && let Some(directory) = executable.parent()
    {
        let installed = directory.join(relative);
        if installed.exists() {
            return installed;
        }
    }
    PathBuf::from(relative)
}

/// Iced intentionally starts with a tiny, deterministic font database instead
/// of scanning the operating system. Load the platform emoji/symbol fonts
/// explicitly so both interface icons and emoji in model output have a real
/// fallback instead of rendering as empty boxes.
fn fallback_font_bytes() -> Vec<Vec<u8>> {
    let candidates = [
        resource_path("assets/NotoColorEmoji.ttf"),
        resource_path("assets/NotoSansSymbols2-Regular.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\seguiemj.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\seguisym.ttf"),
        PathBuf::from("/System/Library/Fonts/Apple Color Emoji.ttc"),
        PathBuf::from("/System/Library/Fonts/Apple Symbols.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/noto/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf"),
        PathBuf::from("/usr/share/fonts/TTF/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/share/fonts/google-noto-emoji-fonts/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/local/share/fonts/NotoColorEmoji.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
    ];

    candidates
        .into_iter()
        .filter_map(|path| fs::read(path).ok())
        .collect()
}

fn load_settings_text() -> Option<String> {
    let settings_path = user_settings_path();
    [
        settings_path.clone(),
        sidecar_path(&settings_path, ".bak"),
        resource_path("config/settings.json"),
    ]
    .into_iter()
    .find_map(|path| {
        let contents = fs::read_to_string(path).ok()?;
        serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&contents)
            .ok()
            .map(|_| contents)
    })
}

fn canonical_code_language(language: &str) -> Option<&'static str> {
    match language.trim().to_ascii_lowercase().as_str() {
        "python" | "py" | "python3" => Some("Python"),
        "rust" | "rs" => Some("Rust"),
        "c" => Some("C"),
        "cpp" | "c++" | "cxx" | "cplusplus" => Some("C++"),
        "cs" | "c#" | "csharp" | "c-sharp" => Some("C#"),
        _ => None,
    }
}

fn check_code(language: String, code: String) -> Result<String, String> {
    let Some(language) = canonical_code_language(&language) else {
        return Err("Code checking supports Python, Rust, C, C++, and C#.".into());
    };
    let directory = std::env::temp_dir().join(format!(
        "locoryn-code-check-{}-{}",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create a temporary check folder: {error}"))?;

    let (file_name, mut command, arguments): (&str, Command, Vec<&str>) = match language {
        "Python" => (
            "snippet.py",
            Command::new("python3"),
            vec!["-m", "py_compile", "snippet.py"],
        ),
        "Rust" => (
            "snippet.rs",
            Command::new("rustc"),
            vec![
                "--crate-type",
                "lib",
                "--emit",
                "metadata",
                "snippet.rs",
                "-o",
                "snippet.rmeta",
            ],
        ),
        "C" => (
            "snippet.c",
            Command::new("cc"),
            vec!["-fsyntax-only", "snippet.c"],
        ),
        "C++" => (
            "snippet.cpp",
            Command::new("c++"),
            vec!["-fsyntax-only", "snippet.cpp"],
        ),
        "C#" => (
            "snippet.cs",
            Command::new("csc"),
            vec!["/nologo", "/target:library", "snippet.cs"],
        ),
        _ => unreachable!(),
    };

    let result = (|| {
        fs::write(directory.join(file_name), code)
            .map_err(|error| format!("Could not prepare the code check: {error}"))?;
        let output = command
            .args(arguments)
            .current_dir(&directory)
            .output()
            .map_err(|error| {
                format!(
                    "{language} checker is unavailable. Install its compiler/interpreter and try again: {error}"
                )
            })?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let details = format!("{stdout}{stderr}")
            .trim()
            .chars()
            .take(4_000)
            .collect::<String>();
        if output.status.success() {
            Ok(if details.is_empty() {
                format!("{language} check passed with no errors.")
            } else {
                format!("{language} check passed:\n{details}")
            })
        } else {
            Err(format!(
                "{language} check found errors{}{}",
                if details.is_empty() { "." } else { ":\n" },
                details
            ))
        }
    })();
    let _ = fs::remove_dir_all(&directory);
    result
}

fn censor_text(input: &str) -> String {
    Censor::from_str(input)
        .with_censor_replacement('#')
        .with_censor_first_character_threshold(Type::INAPPROPRIATE)
        .censor()
}

/// Separates model reasoning from the user-facing answer. Unclosed tags are
/// treated as in-progress reasoning, which keeps streamed chain-of-thought out
/// of the transcript until the closing tag arrives.
fn split_thinking_text(input: &str) -> (String, String) {
    let mut thinking = String::new();
    let mut visible = String::new();
    let mut rest = input;

    while let Some(open) = rest.find("<think>") {
        visible.push_str(&rest[..open]);
        let after_open = &rest[open + "<think>".len()..];
        if let Some(close) = after_open.find("</think>") {
            thinking.push_str(&after_open[..close]);
            rest = &after_open[close + "</think>".len()..];
        } else {
            thinking.push_str(after_open);
            rest = "";
            break;
        }
    }
    visible.push_str(rest);

    (
        thinking.trim().to_string(),
        visible.trim_start().to_string(),
    )
}

fn code_fence_language_alias(language: &str) -> Option<&'static str> {
    match language.to_ascii_lowercase().as_str() {
        "csharp" | "c-sharp" => Some("cs"),
        "cplusplus" | "cxx" => Some("cpp"),
        "golang" => Some("go"),
        "javascriptreact" => Some("jsx"),
        "typescriptreact" => Some("tsx"),
        "objectivec" | "objective-c" => Some("m"),
        "visualbasic" | "vbnet" | "vb.net" => Some("vb"),
        _ => None,
    }
}

/// Syntect accepts a language name or file extension, while models often emit
/// popular aliases such as `csharp`. Normalize only opening fence info strings;
/// code inside a fenced block is left byte-for-byte unchanged.
fn normalize_code_fence_languages(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut open_fence: Option<(u8, usize)> = None;

    for segment in input.split_inclusive('\n') {
        let body_length = segment.trim_end_matches(['\r', '\n']).len();
        let body = &segment[..body_length];
        let line_ending = &segment[body_length..];
        let trimmed = body.trim_start();
        let indentation_length = body.len() - trimmed.len();
        let marker = trimmed.as_bytes().first().copied();
        let marker_length = marker.map_or(0, |marker| {
            trimmed
                .as_bytes()
                .iter()
                .take_while(|character| **character == marker)
                .count()
        });

        if let Some((active_marker, active_length)) = open_fence {
            let is_closing = marker == Some(active_marker)
                && marker_length >= active_length
                && trimmed[marker_length..].trim().is_empty();
            if is_closing {
                open_fence = None;
            }
            output.push_str(segment);
            continue;
        }

        if !matches!(marker, Some(b'`' | b'~')) || marker_length < 3 {
            output.push_str(segment);
            continue;
        }

        open_fence = Some((marker.unwrap(), marker_length));
        let info = &trimmed[marker_length..];
        let leading_space = info.len() - info.trim_start().len();
        let token_start = indentation_length + marker_length + leading_space;
        let token_length = info[leading_space..]
            .find(char::is_whitespace)
            .unwrap_or(info.len() - leading_space);
        let token_end = token_start + token_length;
        let token = &body[token_start..token_end];

        if let Some(alias) = code_fence_language_alias(token) {
            output.push_str(&body[..token_start]);
            output.push_str(alias);
            output.push_str(&body[token_end..]);
            output.push_str(line_ending);
        } else {
            output.push_str(segment);
        }
    }

    output
}

fn parse_markdown_items(input: &str) -> Vec<markdown::Item> {
    let normalized = normalize_code_fence_languages(input);
    markdown::parse(&normalized).collect()
}

fn decode_generation_line(
    input: &str,
) -> Result<(GenerationResponse, Option<String>), serde_json::Error> {
    let value = serde_json::from_str::<serde_json::Value>(input)?;
    let done_reason = value
        .get("done_reason")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let response = serde_json::from_value(value)?;
    Ok((response, done_reason))
}

fn disabled_web_tool_message(input: &str) -> Option<&'static str> {
    let trimmed = input.trim();
    let looks_like_tool_call = (trimmed.starts_with('{') || trimmed.starts_with("```json"))
        && (trimmed.contains("\"web_search\"") || trimmed.contains("\"fetch_webpage\""));
    looks_like_tool_call.then_some(
        "Web search is disabled. Enable it in Settings or with the Web toggle for this chat.",
    )
}

async fn read_response_limited(
    mut response: reqwest::Response,
    maximum_bytes: usize,
) -> Result<Vec<u8>, String> {
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes as u64)
    {
        return Err(format!("response exceeds the {maximum_bytes}-byte limit"));
    }

    let mut bytes = Vec::with_capacity(
        response
            .content_length()
            .unwrap_or_default()
            .min(maximum_bytes as u64) as usize,
    );
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| format!("could not read response: {error}"))?
    {
        if chunk.len() > maximum_bytes.saturating_sub(bytes.len()) {
            return Err(format!("response exceeds the {maximum_bytes}-byte limit"));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn validate_image_dimensions(bytes: &[u8]) -> Result<(), String> {
    let reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Unsupported image: {error}"))?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| format!("Unsupported image: {error}"))?;
    if u64::from(width) * u64::from(height) > MAX_MARKDOWN_IMAGE_PIXELS {
        return Err("Image dimensions are too large to display safely.".to_string());
    }
    Ok(())
}

fn decoded_image_handle(bytes: &[u8]) -> Result<iced::widget::image::Handle, String> {
    if bytes.len() > MAX_MARKDOWN_IMAGE_BYTES {
        return Err("Image is larger than the 12 MB display limit.".to_string());
    }

    validate_image_dimensions(bytes)?;
    let decoded =
        image::load_from_memory(bytes).map_err(|error| format!("Unsupported image: {error}"))?;
    let rgba = decoded.into_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(iced::widget::image::Handle::from_rgba(
        width,
        height,
        rgba.into_raw(),
    ))
}

fn remote_image_url_is_safe(url: &url::Url) -> bool {
    if !matches!(url.scheme(), "http" | "https") || !url.username().is_empty() {
        return false;
    }

    match url.host() {
        Some(url::Host::Domain(domain)) => {
            let domain = domain.trim_end_matches('.').to_ascii_lowercase();
            domain != "localhost" && !domain.ends_with(".localhost")
        }
        Some(url::Host::Ipv4(address)) => {
            !(address.is_private()
                || address.is_loopback()
                || address.is_link_local()
                || address.is_unspecified()
                || address.is_broadcast())
        }
        Some(url::Host::Ipv6(address)) => {
            !(address.is_loopback()
                || address.is_unspecified()
                || address.is_unique_local()
                || address.is_unicast_link_local()
                || matches!(address.to_ipv4_mapped(), Some(v4) if
                    v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_unspecified()))
        }
        None => false,
    }
}

async fn load_markdown_image(url: String) -> Result<iced::widget::image::Handle, String> {
    let bytes = if let Some(data) = url.strip_prefix("data:image/") {
        let (metadata, encoded) = data
            .split_once(',')
            .ok_or_else(|| "Malformed embedded image.".to_string())?;
        if !metadata
            .split(';')
            .any(|part| part.eq_ignore_ascii_case("base64"))
        {
            return Err("Only base64 embedded images are supported.".to_string());
        }
        if encoded.len() > MAX_MARKDOWN_IMAGE_BYTES * 2 {
            return Err("Embedded image is too large.".to_string());
        }
        BASE64
            .decode(encoded)
            .map_err(|error| format!("Could not decode embedded image: {error}"))?
    } else {
        let parsed =
            url::Url::parse(&url).map_err(|error| format!("Invalid image URL: {error}"))?;
        if !remote_image_url_is_safe(&parsed) {
            return Err("Blocked an unsafe or unsupported image URL.".to_string());
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| format!("Could not prepare image request: {error}"))?;
        let mut current = parsed;
        let mut response = None;
        for redirect_count in 0..=5 {
            validate_public_url(&current)
                .await
                .map_err(|error| error.user_message().to_string())?;
            let next = client
                .get(current.clone())
                .header(
                    reqwest::header::USER_AGENT,
                    concat!("locoryn/", env!("CARGO_PKG_VERSION"), " image-preview"),
                )
                .send()
                .await
                .map_err(|error| format!("Could not load image: {error}"))?;
            if !next.status().is_redirection() {
                response = Some(
                    next.error_for_status()
                        .map_err(|error| format!("Could not load image: {error}"))?,
                );
                break;
            }
            if redirect_count == 5 {
                return Err("Remote image redirected too many times.".to_string());
            }
            let location = next
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "Remote image returned an invalid redirect.".to_string())?;
            current = current
                .join(location)
                .map_err(|_| "Remote image returned an invalid redirect.".to_string())?;
        }
        read_response_limited(
            response.ok_or_else(|| "Could not load remote image.".to_string())?,
            MAX_MARKDOWN_IMAGE_BYTES,
        )
        .await
        .map_err(|_| "Remote image is larger than the 12 MB display limit.".to_string())?
    };

    decoded_image_handle(&bytes)
}

fn convert_port_to_u16(port: String) -> u16 {
    match port.parse::<u16>() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Invalid port number: {}", port);
            11434
        }
    }
}

fn load_chat_image(path: &Path) -> Result<ChatImage, String> {
    let bytes = fs::read(path).map_err(|error| format!("Could not read image: {error}"))?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err("Images must be smaller than 20 MB.".to_string());
    }
    let preview_handle = decoded_image_handle(&bytes)?;
    let mime_type = match path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    };
    Ok(ChatImage {
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("image")
            .to_string(),
        mime_type: mime_type.to_string(),
        bytes,
        preview_handle,
    })
}

fn paste_chat_image() -> Result<ChatImage, String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("Could not open clipboard: {error}"))?;
    let image_data = clipboard
        .get_image()
        .map_err(|_| "The clipboard does not contain an image.".to_string())?;
    let rgba = image::RgbaImage::from_raw(
        image_data.width as u32,
        image_data.height as u32,
        image_data.bytes.into_owned(),
    )
    .ok_or_else(|| "Clipboard image data was invalid.".to_string())?;
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .map_err(|error| format!("Could not prepare clipboard image: {error}"))?;
    let preview_handle = decoded_image_handle(&bytes)?;
    Ok(ChatImage {
        name: "Pasted image.png".to_string(),
        mime_type: "image/png".to_string(),
        bytes,
        preview_handle,
    })
}

fn copy_image_file(path: &str) -> Result<(), String> {
    let decoded = image::open(path)
        .map_err(|error| format!("Could not open image: {error}"))?
        .into_rgba8();
    let (width, height) = decoded.dimensions();
    let mut clipboard =
        arboard::Clipboard::new().map_err(|error| format!("Could not open clipboard: {error}"))?;
    clipboard
        .set_image(arboard::ImageData {
            width: width as usize,
            height: height as usize,
            bytes: std::borrow::Cow::Owned(decoded.into_raw()),
        })
        .map_err(|error| format!("Could not copy image: {error}"))
}

fn version_components(version: &str) -> Option<Vec<u64>> {
    let version = version.trim().trim_start_matches(['v', 'V']);
    let stable = version.split(['-', '+']).next()?;
    let components = stable
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    (!components.is_empty()).then_some(components)
}

fn compare_versions(left: &str, right: &str) -> Option<ComparisonOrdering> {
    let mut left = version_components(left)?;
    let mut right = version_components(right)?;
    let length = left.len().max(right.len());
    left.resize(length, 0);
    right.resize(length, 0);
    Some(left.cmp(&right))
}

async fn fetch_latest_app_release() -> Result<AppUpdateInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent(concat!("locoryn/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("Could not prepare the update check: {error}"))?;
    let response = client
        .get(UPDATE_RELEASE_API)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| format!("Could not check for updates: {error}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "The update service returned HTTP {}.",
            response.status()
        ));
    }
    let release = response
        .json::<serde_json::Value>()
        .await
        .map_err(|error| format!("Could not read the update response: {error}"))?;
    let latest_version = release
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|version| version_components(version).is_some())
        .ok_or_else(|| "The update service returned an invalid version.".to_string())?
        .trim_start_matches(['v', 'V'])
        .to_string();
    let release_url = release
        .get("html_url")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| url::Url::parse(value).ok())
        .filter(|url| url.scheme() == "https" && url.host_str() == Some("github.com"))
        .map(|url| url.to_string())
        .unwrap_or_else(|| UPDATE_RELEASE_PAGE.to_string());

    Ok(AppUpdateInfo {
        latest_version,
        release_url,
    })
}

fn open_url(url: String) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                webbrowser::open(&url).map_err(|error| format!("Could not open link: {error}"))
            })
            .await
            .map_err(|error| format!("Could not open link: {error}"))?
        },
        Message::UrlOpened,
    )
}

fn send_chat_notice(
    sender: &crossbeam_channel::Sender<(String, DebugMessage)>,
    chat_id: &str,
    message: DebugMessage,
) {
    let _ = sender.send((chat_id.to_string(), message));
}

fn append_failed_response(
    chat_history: &Arc<Mutex<CurrentChat>>,
    model: Option<String>,
    message: String,
) {
    chat_history
        .lock()
        .unwrap()
        .push_message(Correspondence::Bot {
            text: message,
            model,
            thinking_seconds: None,
            sources: Vec::new(),
            web_search_used: false,
        });
}

async fn wait_until_cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

impl Program {
    fn new_chat_id() -> String {
        format!(
            "chat-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        )
    }

    fn current_active_prompt(&self) -> Option<&ActivePrompt> {
        self.active_prompts.get(&self.current_chat_id)
    }

    fn current_chat_is_processing(&self) -> bool {
        self.current_active_prompt().is_some()
    }

    fn prompt_progress(&self) -> f32 {
        let elapsed = self
            .active_prompts
            .values()
            .map(|job| job.started_at.elapsed())
            .min()
            .unwrap_or_default()
            .as_secs_f32();
        let phase = (elapsed / 1.4).fract();
        let wave = 1.0 - (phase * 2.0 - 1.0).abs();
        // Smooth the turnarounds so the indicator glides instead of bouncing.
        let eased = wave * wave * (3.0 - 2.0 * wave);
        0.24 + eased * 0.52
    }

    fn check_for_updates(&mut self) -> Task<Message> {
        self.app_update_state = AppUpdateState::Checking;
        Task::perform(fetch_latest_app_release(), Message::AppUpdateChecked)
    }

    fn begin_page_transition(&mut self) {
        self.page_reveal = 0.0;
    }

    fn trigger_settings_feedback(&mut self, target: SettingsFeedbackTarget) {
        let should_restart = self.settings_feedback.is_none_or(|(active, started_at)| {
            active != target || started_at.elapsed() >= Duration::from_millis(170)
        });
        if should_restart {
            self.settings_feedback = Some((target, Instant::now()));
        }
    }

    fn settings_feedback(&self, target: SettingsFeedbackTarget) -> f32 {
        let Some((active, started_at)) = self.settings_feedback else {
            return 0.0;
        };
        if active != target {
            return 0.0;
        }

        let progress = (started_at.elapsed().as_secs_f32()
            / (SETTINGS_FEEDBACK_DURATION_MS as f32 / 1_000.0))
            .clamp(0.0, 1.0);
        if progress >= 1.0 {
            return 0.0;
        }

        (progress * std::f32::consts::PI * 3.0).sin().abs() * (1.0 - progress).powi(2)
    }

    fn queue_missing_markdown_images(&mut self) -> Task<Message> {
        let mut urls = HashSet::new();
        let mut collect = |items: &[markdown::Item]| {
            for item in items {
                if let markdown::Item::Image { url, .. } = item {
                    urls.insert(url.clone());
                }
            }
        };

        for items in &self.chat_markdown_cache {
            collect(items);
        }
        for job in self.active_prompts.values() {
            collect(&job.parsed_markdown);
        }
        for response in self.vision_responses.values() {
            collect(&response.markdown);
        }

        let mut tasks = Vec::new();
        for url in urls {
            if self.markdown_images.contains_key(&url) {
                continue;
            }

            let is_supported = url.starts_with("data:image/")
                || url::Url::parse(&url)
                    .ok()
                    .is_some_and(|parsed| remote_image_url_is_safe(&parsed));
            if !is_supported {
                self.markdown_images.insert(
                    url,
                    MarkdownImageState::Failed(
                        "Unsupported image address. Use HTTPS or an embedded image.".to_string(),
                    ),
                );
                continue;
            }

            self.markdown_images
                .insert(url.clone(), MarkdownImageState::Loading);
            let request_url = url.clone();
            tasks.push(Task::perform(
                load_markdown_image(request_url),
                move |result| Message::MarkdownImageLoaded { url, result },
            ));
        }

        Task::batch(tasks)
    }

    fn advance_ui_motion(&mut self) {
        self.ui_motion = (self.ui_motion + 0.0045).fract();
        self.page_reveal += (1.0 - self.page_reveal) * 0.06;
        if self.page_reveal > 0.998 {
            self.page_reveal = 1.0;
        }

        let target = if self.chat_menu_open { 1.0 } else { 0.0 };
        self.sidebar_animation += (target - self.sidebar_animation) * 0.08;
        if (target - self.sidebar_animation).abs() < 0.002 {
            self.sidebar_animation = target;
        }
    }

    fn needs_frame_updates(&self) -> bool {
        let sidebar_target = if self.chat_menu_open { 1.0 } else { 0.0 };
        !self.active_prompts.is_empty()
            || self.is_generating_image
            || self.page_reveal < 1.0
            || self.settings_feedback.is_some_and(|(_, started_at)| {
                started_at.elapsed() < Duration::from_millis(SETTINGS_FEEDBACK_DURATION_MS)
            })
            || (sidebar_target - self.sidebar_animation).abs() >= 0.002
            || self
                .markdown_images
                .values()
                .any(|state| matches!(state, MarkdownImageState::Loading))
    }

    fn clear_open_chat(&mut self) {
        self.user_information.chat_history = Arc::new(Mutex::new(CurrentChat {
            chats: vec![],
            messages: vec![],
            bot_responding: false,
        }));
        self.chat_messages_cache.clear();
        self.chat_thinking_cache.clear();
        self.chat_visible_text_cache.clear();
        self.chat_markdown_cache.clear();
        self.chat_model_name_cache.clear();
        self.last_copied_text = None;
        self.last_copied_at = None;
        self.expanded_thinking.clear();
        self.expanded_sources.clear();
        self.open_chat_dirty = false;
    }

    fn save_open_chat(&mut self) {
        if self.temporary_chat || !self.open_chat_dirty {
            return;
        }
        let chat = self.user_information.chat_history.lock().unwrap().clone();
        let profile = self
            .saved_chats
            .iter()
            .find(|chat| chat.id == self.current_chat_id)
            .map(chat_profile_id)
            .map(str::to_string)
            .unwrap_or_else(|| self.active_profile_id.clone());
        self.save_chat_snapshot(
            self.current_chat_id.clone(),
            &chat,
            self.web_search_for_chat,
            profile,
        );
        self.open_chat_dirty = false;
    }

    fn save_chat_snapshot(
        &mut self,
        id: String,
        chat: &CurrentChat,
        web_search_enabled: bool,
        profile: String,
    ) {
        if chat.messages.is_empty() {
            return;
        }
        let title = chat
            .messages
            .iter()
            .find_map(|message| match message {
                Correspondence::User { text, .. } => Some(text.trim().chars().take(42).collect()),
                _ => None,
            })
            .filter(|title: &String| !title.is_empty())
            .unwrap_or_else(|| "New chat".into());
        let mut saved = SavedChat::from_current(id, title, chat, web_search_enabled, profile);
        if let Some(existing) = self.saved_chats.iter_mut().find(|item| item.id == saved.id) {
            saved.pinned = existing.pinned;
            // A chat never changes profile when it is updated.
            if existing.profile.is_some() {
                saved.profile = existing.profile.clone();
            }
            *existing = saved;
        } else {
            // New chats appear after the pinned section. Updating or opening an
            // existing chat deliberately leaves it at its current position.
            let insert_at = self
                .saved_chats
                .iter()
                .position(|chat| !chat.pinned)
                .unwrap_or(self.saved_chats.len());
            self.saved_chats.insert(insert_at, saved);
        }
        self.persist_saved_chats();
    }

    fn persist_saved_chats(&mut self) {
        if let Err(error) =
            write_json_safely(&self.chat_storage_dir.join("chats.json"), &self.saved_chats)
        {
            self.set_debug_message(DebugMessage {
                message: format!("Could not update saved chats: {error}"),
                is_error: true,
            });
        }
    }

    fn persist_current_chat_web_search_setting(&mut self) {
        if self.temporary_chat {
            if let Some(session) = self.temporary_chats.get_mut(&self.current_chat_id) {
                session.web_search_enabled = self.web_search_for_chat;
            }
            return;
        }

        if let Some(chat) = self
            .saved_chats
            .iter_mut()
            .find(|chat| chat.id == self.current_chat_id)
        {
            chat.web_search_enabled = Some(self.web_search_for_chat);
            self.persist_saved_chats();
        }
    }

    fn persist_boolean_setting(&mut self, key: &str, value: bool) {
        self.persist_setting_value(key, serde_json::Value::Bool(value));
    }

    fn persist_web_search_settings(&mut self) {
        match serde_json::to_value(&self.web_search_settings) {
            Ok(value) => self.persist_setting_value("web_search", value),
            Err(error) => self.set_debug_message(DebugMessage {
                message: format!("Could not save web-search settings: {error}"),
                is_error: true,
            }),
        }
    }

    fn persist_dynamic_prompt_settings(&mut self) {
        match serde_json::to_value(&self.dynamic_prompt_settings) {
            Ok(value) => self.persist_setting_value("dynamic_prompt", value),
            Err(error) => self.set_debug_message(DebugMessage {
                message: format!("Could not save dynamic prompt settings: {error}"),
                is_error: true,
            }),
        }
    }

    fn persist_ui_layout(&mut self) {
        match serde_json::to_value(&self.ui_layout) {
            Ok(value) => self.persist_setting_value("ui_layout", value),
            Err(error) => self.set_debug_message(DebugMessage {
                message: format!("Could not save UI layout: {error}"),
                is_error: true,
            }),
        }
    }

    fn persist_setting_value(&mut self, key: &str, value: serde_json::Value) {
        self.pending_settings.insert(key.to_string(), value);
        self.settings_dirty_at = Some(Instant::now());
    }

    fn flush_pending_settings_if_ready(&mut self) {
        let Some(dirty_at) = self.settings_dirty_at else {
            return;
        };
        if dirty_at.elapsed() < Duration::from_millis(SETTINGS_SAVE_DEBOUNCE_MS) {
            return;
        }

        let pending = std::mem::take(&mut self.pending_settings);
        let mut settings: serde_json::Map<String, serde_json::Value> = load_settings_text()
            .and_then(|data| serde_json::from_str::<serde_json::Value>(&data).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        settings.extend(pending.clone());
        let settings_path = user_settings_path();
        let result = write_json_safely(&settings_path, &settings);
        if let Err(error) = result {
            self.pending_settings.extend(pending);
            self.settings_dirty_at = Some(Instant::now());
            self.set_debug_message(DebugMessage {
                message: format!("Could not save setting: {error}"),
                is_error: true,
            });
        } else {
            self.settings_dirty_at = None;
        }
    }

    fn persist_chat_storage_dir(&mut self) -> Result<(), String> {
        let settings_path = chat_location_settings_path();
        let value = serde_json::json!({
            "chat_storage_dir": self.chat_storage_dir.to_string_lossy()
        });
        write_json_safely(&settings_path, &value)
    }

    fn persist_profiles(&mut self) {
        let registry = ProfileRegistry {
            profiles: self.profiles.clone(),
            active_profile_id: self.active_profile_id.clone(),
        };
        if let Err(error) = write_json_safely(&profiles_path(), &registry) {
            self.set_debug_message(DebugMessage {
                message: format!("Could not save profiles: {error}"),
                is_error: true,
            });
        }
    }

    fn profile_display_name(&self, profile_id: &str) -> String {
        self.profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .map(|profile| profile.name.clone())
            .unwrap_or_else(|| LEGACY_PROFILE_NAME.to_string())
    }

    fn active_profile_name(&self) -> String {
        self.profile_display_name(&self.active_profile_id)
    }

    /// The only profile strings that may reach the model: its user name and
    /// its extra instructions. The display name stays private.
    fn profile_prompt_extras(&self, profile_id: &str) -> (String, String) {
        self.profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .map(|profile| {
                (
                    profile.user_name.clone(),
                    profile.custom_instructions.clone(),
                )
            })
            .unwrap_or_default()
    }

    /// Opens the newest chat belonging to a profile, or starts a fresh one
    /// when the profile has no chats yet.
    fn open_latest_chat_for_profile(&mut self, profile_id: &str) -> Task<Message> {
        let next_saved = self
            .saved_chats
            .iter()
            .filter(|chat| chat_profile_id(chat) == profile_id)
            .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
            .map(|chat| chat.id.clone());
        if let Some(id) = next_saved {
            return self.open_chat(id);
        }

        let next_temporary = self
            .temporary_chats
            .iter()
            .find(|(_, session)| session.profile_id == profile_id)
            .map(|(chat_id, _)| chat_id.clone());
        if let Some(id) = next_temporary {
            return self.open_chat(id);
        }

        self.temporary_chat = false;
        self.current_chat_id = Self::new_chat_id();
        self.web_search_for_chat = self.web_search_settings.enabled;
        self.clear_open_chat();
        Task::none()
    }

    fn switch_profile(&mut self, profile_id: String) -> Task<Message> {
        if !self.profiles.iter().any(|profile| profile.id == profile_id)
            || profile_id == self.active_profile_id
        {
            return Task::none();
        }
        self.save_open_chat();
        self.active_profile_id = profile_id.clone();
        self.persist_profiles();
        let open_task = self.open_latest_chat_for_profile(&profile_id);
        self.begin_page_transition();
        open_task
    }

    fn open_chat(&mut self, id: String) -> Task<Message> {
        self.save_open_chat();
        let running_history = self
            .active_prompts
            .get(&id)
            .map(|job| Arc::clone(&job.chat_history));
        let running_settings = self
            .active_prompts
            .get(&id)
            .map(|job| (job.temporary, job.web_search_enabled));
        let temporary_history = self
            .temporary_chats
            .get(&id)
            .map(|chat| Arc::clone(&chat.chat_history));
        let temporary_settings = self
            .temporary_chats
            .get(&id)
            .map(|chat| (true, chat.web_search_enabled));
        let chat_settings = running_settings.or(temporary_settings);
        let saved = self.saved_chats.iter().find(|chat| chat.id == id).cloned();
        let saved_web_search_enabled = saved.as_ref().and_then(|chat| chat.web_search_enabled);
        if let Some(chat_history) = running_history
            .or(temporary_history)
            .or_else(|| saved.map(|chat| Arc::new(Mutex::new(chat.to_current()))))
        {
            self.current_chat_id = id;
            if let Some((_, shown_at)) = self.chat_notices.get_mut(&self.current_chat_id) {
                *shown_at = Instant::now();
            }
            self.temporary_chat = chat_settings
                .map(|(temporary, _)| temporary)
                .unwrap_or(false);
            self.web_search_for_chat = chat_settings
                .map(|(_, web_search_enabled)| web_search_enabled)
                .or(saved_web_search_enabled)
                .unwrap_or(self.web_search_settings.enabled);
            self.user_information.chat_history = chat_history;
            self.open_chat_dirty = false;
            // Rendering caches are positional and belong only to the
            // previously open chat.
            self.chat_messages_cache.clear();
            self.chat_thinking_cache.clear();
            self.chat_visible_text_cache.clear();
            self.chat_markdown_cache.clear();
            self.chat_model_name_cache.clear();
            self.expanded_thinking.clear();
            self.expanded_sources.clear();
            self.last_copied_text = None;
            self.last_copied_at = None;
            self.refresh_chat_markdown_cache();
            self.begin_page_transition();
        }
        self.queue_missing_markdown_images()
    }

    fn set_debug_message(&mut self, debug_message: DebugMessage) {
        let has_message = !debug_message.message.trim().is_empty();

        self.debug_message = debug_message;
        self.debug_message_set_at = if has_message {
            Some(Instant::now())
        } else {
            None
        };
    }

    fn clear_debug_message_if_old(&mut self) {
        if let Some(set_at) = self.debug_message_set_at
            && set_at.elapsed() >= Duration::from_secs(15)
        {
            self.debug_message.message.clear();
            self.debug_message.is_error = false;
            self.debug_message_set_at = None;
        }
        let current_chat_id = &self.current_chat_id;
        self.chat_notices.retain(|chat_id, (_, set_at)| {
            chat_id != current_chat_id || set_at.elapsed() < Duration::from_secs(15)
        });
    }

    fn current_debug_message(&self) -> &DebugMessage {
        self.chat_notices
            .get(&self.current_chat_id)
            .map(|(message, _)| message)
            .unwrap_or(&self.debug_message)
    }

    fn refresh_chat_markdown_cache(&mut self) {
        let chat_history = Arc::clone(&self.user_information.chat_history);
        let chat_history = chat_history.lock().unwrap();
        let message_count = chat_history.messages.len();
        self.chat_markdown_cache.truncate(message_count);
        self.chat_model_name_cache.truncate(message_count);
        self.chat_thinking_cache.truncate(message_count);
        self.chat_visible_text_cache.truncate(message_count);

        for (index, message) in chat_history.messages.iter().enumerate() {
            if index < self.chat_markdown_cache.len() {
                if self.chat_model_name_cache[index].is_none()
                    && let Correspondence::Bot { model, .. } = message
                {
                    self.chat_model_name_cache[index] = model.clone();
                }
                continue;
            }

            match message {
                Correspondence::User { .. } => {
                    self.chat_markdown_cache.push(Vec::new());
                    self.chat_model_name_cache.push(None);
                    self.chat_thinking_cache.push(String::new());
                    self.chat_visible_text_cache.push(String::new());
                }

                Correspondence::Bot { text, model, .. } => {
                    let (thinking, visible_text) = split_thinking_text(text);
                    self.chat_markdown_cache
                        .push(parse_markdown_items(&visible_text));
                    self.chat_thinking_cache.push(thinking);
                    self.chat_visible_text_cache.push(visible_text);
                    let model_name = model.clone().or_else(|| Some("Unknown model".to_string()));
                    self.chat_model_name_cache.push(model_name);
                }
            }
        }
        self.chat_messages_cache.clone_from(&chat_history.messages);
    }

    fn drain_live_updates(&mut self) -> bool {
        let filtering = self.app_state.filtering;
        let current_chat_id = self.current_chat_id.clone();
        let mut changed = false;
        for (chat_id, job) in self.active_prompts.iter_mut() {
            let had_thinking = !job.thinking_text.is_empty();
            while let Ok(state) = job.web_search_state_receiver.try_recv() {
                job.web_search_state = state;
                changed = true;
            }
            while let Ok(render) = job.render_receiver.try_recv() {
                job.response_text = render.text;
                job.thinking_text = render.thinking;
                job.parsed_markdown = render.markdown;
                changed = true;
            }
            if job.web_progress_receiver.has_changed().unwrap_or(false) {
                let mut progress = job.web_progress_receiver.borrow_and_update().clone();
                if filtering {
                    progress.thinking = censor_text(&progress.thinking);
                    progress.answer = censor_text(&progress.answer);
                }
                job.thinking_text = progress.thinking;
                job.response_text = if job.thinking_text.is_empty() {
                    progress.answer.clone()
                } else {
                    format!("<think>{}</think>{}", job.thinking_text, progress.answer)
                };
                job.parsed_markdown = parse_markdown_items(&progress.answer);
                changed = true;
            }
            if !had_thinking && !job.thinking_text.is_empty() && chat_id == &current_chat_id {
                self.expanded_thinking.insert(usize::MAX);
            }
        }
        while let Ok((chat_id, notice)) = self.chat_notice_receiver.try_recv() {
            self.chat_notices.insert(chat_id, (notice, Instant::now()));
            changed = true;
        }
        changed
    }

    fn clear_copy_feedback_if_old(&mut self) {
        if let Some(copied_at) = self.last_copied_at
            && copied_at.elapsed() >= Duration::from_millis(1400)
        {
            self.last_copied_text = None;
            self.last_copied_at = None;
        }
    }

    fn apply_response_metadata(
        chat: &mut CurrentChat,
        response_start_index: usize,
        model_name: &str,
        elapsed_seconds: u64,
    ) {
        if let Some(Correspondence::Bot {
            model: stored_model,
            thinking_seconds,
            ..
        }) = chat
            .messages
            .iter_mut()
            .skip(response_start_index)
            .rev()
            .find(|message| matches!(message, Correspondence::Bot { .. }))
        {
            if stored_model.is_none() {
                *stored_model = Some(model_name.to_string());
            }
            if thinking_seconds.is_none() {
                *thinking_seconds = Some(elapsed_seconds);
            }
        }
    }

    fn finalize_response_metadata(job: &ActivePrompt) {
        let elapsed_seconds = job.started_at.elapsed().as_secs().max(1);
        if let Ok(mut chat) = job.chat_history.lock() {
            Self::apply_response_metadata(
                &mut chat,
                job.response_start_index,
                &job.model_name,
                elapsed_seconds,
            );
        }
    }

    fn finish_prompt(&mut self, chat_id: &str) {
        let Some(job) = self.active_prompts.remove(chat_id) else {
            return;
        };
        Self::finalize_response_metadata(&job);

        let completed_chat = job.chat_history.lock().unwrap().clone();
        if job.temporary {
            self.temporary_chats.insert(
                chat_id.to_string(),
                TemporaryChatSession {
                    chat_history: Arc::clone(&job.chat_history),
                    web_search_enabled: job.web_search_enabled,
                    profile_id: job.profile_id.clone(),
                },
            );
        } else {
            self.save_chat_snapshot(
                chat_id.to_string(),
                &completed_chat,
                job.web_search_enabled,
                job.profile_id.clone(),
            );
        }

        if job.had_image
            && let Some(response) = completed_chat
                .messages
                .iter()
                .skip(job.response_start_index)
                .rev()
                .find_map(|message| match message {
                    Correspondence::Bot { text, .. } => Some(text.as_str()),
                    Correspondence::User { .. } => None,
                })
        {
            let (_, visible) = split_thinking_text(response);
            self.vision_responses.insert(
                chat_id.to_string(),
                VisionResponse {
                    markdown: parse_markdown_items(&visible),
                },
            );
        }

        if self.current_chat_id == chat_id {
            self.user_information.chat_history = Arc::clone(&job.chat_history);
            self.expanded_thinking.remove(&usize::MAX);
            self.refresh_chat_markdown_cache();
            self.open_chat_dirty = false;
        }
    }

    fn prompt(&mut self, mut prompt: String) -> Task<Message> {
        if self.user_information.model.is_none() {
            Channels::send_request_to_channel(
                Arc::clone(&self.channels.debug_channel),
                DebugMessage {
                    message: "Model selected is invalid, have you selected a model?".to_string(),
                    is_error: true,
                },
            );
            println!("Model is None");
            return Task::none();
        }

        if self.app_state.filtering {
            prompt = censor_text(&prompt);
        }

        let model_name = self.user_information.model.clone().unwrap();
        let started_at = Instant::now();

        let cancel = Arc::new(AtomicBool::new(false));

        // Keep only the newest full Markdown snapshot. A long response can
        // otherwise queue many increasingly large copies while its chat is in
        // the background.
        let (render_sender, render_receiver) = crossbeam_channel::bounded(1);
        let render_receiver_for_renderer = render_receiver.clone();

        // Backpressure bounds token memory if highlighting a large response is
        // temporarily slower than Ollama's stream.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<GenerationResponse>(64);
        let batch_tokens = self.batch_tokens;
        let fast_streaming = self.fast_streaming;
        std::thread::spawn(move || {
            fn render(
                buffer: &str,
                render_sender: &crossbeam_channel::Sender<LiveRender>,
                render_receiver: &crossbeam_channel::Receiver<LiveRender>,
            ) {
                let (thinking, visible) = split_thinking_text(buffer);
                let render = LiveRender {
                    text: buffer.to_string(),
                    thinking,
                    markdown: parse_markdown_items(&visible),
                };
                while render_receiver.try_recv().is_ok() {}
                // A disconnected receiver means the completed job has already
                // been reconciled into its chat history.
                let _ = render_sender.try_send(render);
            }

            let mut buffer = String::new();
            let mut last_render_time = Instant::now();
            let mut total_tokens = 0;

            while let Some(token) = rx.blocking_recv() {
                buffer.push_str(&token.response);

                total_tokens += 1;

                if fast_streaming {
                    // "Fast" remains visually immediate without reparsing the
                    // entire growing answer more than roughly once per frame.
                    if last_render_time.elapsed().as_millis() < LIVE_RENDER_MS {
                        continue;
                    }
                } else if total_tokens < batch_tokens
                    && last_render_time.elapsed().as_millis() < 250
                {
                    continue;
                }

                total_tokens = 0;
                last_render_time = Instant::now();

                render(&buffer, &render_sender, &render_receiver_for_renderer);
            }

            if !buffer.is_empty() {
                render(&buffer, &render_sender, &render_receiver_for_renderer);
            }
        });

        // The prompt belongs to the chat's own profile, not whichever profile
        // happens to be active when the response finishes.
        let prompt_profile_id = if self.temporary_chat {
            self.temporary_chats
                .get(&self.current_chat_id)
                .map(|session| session.profile_id.clone())
                .unwrap_or_else(|| self.active_profile_id.clone())
        } else {
            self.saved_chats
                .iter()
                .find(|chat| chat.id == self.current_chat_id)
                .map(chat_profile_id)
                .map(str::to_string)
                .unwrap_or_else(|| self.active_profile_id.clone())
        };
        let (prompt_user_name, prompt_profile_instructions) =
            self.profile_prompt_extras(&prompt_profile_id);

        let system_prompt: Option<String> = SystemPrompt::get_current(self).map(|base| {
            self.dynamic_prompt_settings.apply(
                &base,
                Local::now(),
                &prompt_user_name,
                &prompt_profile_instructions,
            )
        });

        if system_prompt.is_none() {
            Channels::send_request_to_channel(
                Arc::clone(&self.channels.debug_channel),
                DebugMessage {
                    message: "Could not get system prompt, is it selected?".to_string(),
                    is_error: true,
                },
            );
            return Task::none();
        }

        // Clone the attachment into the request/chat first. The composer owns its
        // copy until the submission has been accepted, avoiding a transient blank
        // preview while the async request is being prepared.
        let attached_images = self.pending_images.clone();
        let had_image = !attached_images.is_empty();
        let logging = self.app_state.logging;
        let filtering = self.app_state.filtering;
        let user_info = self.user_information.clone();
        let channels = self.channels.clone();
        let web_search_enabled = self.web_search_for_chat;
        let mut web_search_settings = self.web_search_settings.clone();
        web_search_settings.enabled = web_search_enabled;
        let (web_search_state_sender, web_search_state_receiver) = crossbeam_channel::unbounded();
        let (web_progress_sender, web_progress_receiver) =
            tokio::sync::watch::channel(ToolLoopProgress::default());
        let chat_id = self.current_chat_id.clone();
        let completion_chat_id = chat_id.clone();
        let notice_chat_id = chat_id.clone();
        let chat_notice_sender = self.chat_notice_sender.clone();
        self.chat_notices.remove(&chat_id);

        let response_start_index = {
            let mut chat = user_info.chat_history.lock().unwrap();
            chat.push_message(Correspondence::User {
                text: prompt.clone(),
                images: attached_images.clone(),
            });
            chat.messages.len()
        };
        self.open_chat_dirty = true;

        self.refresh_chat_markdown_cache();
        self.pending_images.clear();
        user_info.chat_history.lock().unwrap().bot_responding = true;
        if self.temporary_chat {
            self.temporary_chats.remove(&chat_id);
        }
        self.active_prompts.insert(
            chat_id,
            ActivePrompt {
                chat_history: Arc::clone(&user_info.chat_history),
                response_text: String::new(),
                thinking_text: String::new(),
                parsed_markdown: parse_markdown_items("Waiting for bot..."),
                render_receiver,
                web_search_state: WebSearchState::Idle,
                web_search_state_receiver,
                web_progress_receiver,
                cancel: Arc::clone(&cancel),
                model_name,
                started_at,
                response_start_index,
                had_image,
                web_search_enabled,
                temporary: self.temporary_chat,
                profile_id: prompt_profile_id,
            },
        );

        Task::perform(
            async move {
                println!("Received prompt: {}", prompt.clone());

                let system_prompt: String = system_prompt.unwrap();
                let ip = user_info.ip_address.clone();
                let to_send_prompt: String = if user_info.current_chat_history_enabled {
                    conversation_context_prompt(
                        &user_info.chat_history.lock().unwrap().unravel(),
                        &prompt_user_name,
                        &prompt,
                    )
                } else {
                    prompt.clone()
                };

                if web_search_enabled {
                    let provider = match create_search_provider(&web_search_settings) {
                        Ok(provider) => provider,
                        Err(error) => {
                            let api_key = web_search_settings.resolved_api_key();
                            let message = error.detailed_user_message(api_key.as_deref());
                            let _ = web_search_state_sender.send(WebSearchState::Failed {
                                message: message.clone(),
                            });
                            send_chat_notice(
                                &chat_notice_sender,
                                &notice_chat_id,
                                DebugMessage {
                                    message: message.clone(),
                                    is_error: true,
                                },
                            );
                            user_info.chat_history.lock().unwrap().push_message(
                                Correspondence::Bot {
                                    text: format!("Web search could not start: {message}"),
                                    model: user_info.model.clone(),
                                    thinking_seconds: None,
                                    sources: Vec::new(),
                                    web_search_used: true,
                                },
                            );
                            user_info.chat_history.lock().unwrap().bot_responding = false;
                            return;
                        }
                    };
                    let result = run_tool_loop(ToolLoopRequest {
                        ollama_url: format!("http://{}:{}/api/chat", ip.ip, ip.port),
                        model: user_info.model.clone().unwrap(),
                        prompt: to_send_prompt,
                        system_prompt: system_prompt.clone(),
                        temperature: user_info.temperature / 10.0,
                        context_tokens: user_info.context_tokens,
                        max_response_tokens: user_info.max_response_tokens,
                        images: attached_images
                            .iter()
                            .map(|image| BASE64.encode(&image.bytes))
                            .collect(),
                        thinking: user_info.thinking_level.api_value(),
                        settings: web_search_settings.clone(),
                        provider,
                        state_sender: web_search_state_sender.clone(),
                        progress_sender: web_progress_sender,
                        cancel: Arc::clone(&cancel),
                    })
                    .await;

                    match result {
                        Ok(result) => {
                            let complete_response = if result.thinking.trim().is_empty() {
                                result.answer
                            } else {
                                format!("<think>{}</think>{}", result.thinking, result.answer)
                            };
                            let complete_response = if filtering {
                                censor_text(&complete_response)
                            } else {
                                complete_response
                            };
                            let _ = tx
                                .send(GenerationResponse {
                                    model: user_info.model.clone().unwrap(),
                                    created_at: Local::now().to_rfc3339(),
                                    response: complete_response.clone(),
                                    done: true,
                                    context: None,
                                    total_duration: None,
                                    load_duration: None,
                                    prompt_eval_count: None,
                                    prompt_eval_duration: None,
                                    eval_count: None,
                                    eval_duration: None,
                                    thinking: None,
                                    logprobs: None,
                                })
                                .await;
                            if logging {
                                Channels::send_request_to_channel(
                                    Arc::clone(&channels.logging_channel),
                                    Log::create_with_current_time(
                                        filtering,
                                        user_info.model.clone(),
                                        vec![complete_response.clone()],
                                        Some(system_prompt),
                                        prompt.clone(),
                                    ),
                                );
                            }
                            if user_info.current_chat_history_enabled {
                                let (_, visible_response) = split_thinking_text(&complete_response);
                                user_info
                                    .chat_history
                                    .lock()
                                    .unwrap()
                                    .generate_and_push(prompt.clone(), visible_response);
                            }
                            user_info.chat_history.lock().unwrap().push_message(
                                Correspondence::Bot {
                                    text: complete_response,
                                    model: user_info.model.clone(),
                                    thinking_seconds: None,
                                    sources: result.sources,
                                    web_search_used: true,
                                },
                            );
                        }
                        Err(crate::web_search::WebSearchError::Cancelled) => {}
                        Err(error) => {
                            let api_key = web_search_settings.resolved_api_key();
                            let message = error.detailed_user_message(api_key.as_deref());
                            eprintln!(
                                "Web-search failure: {}",
                                error.diagnostic(api_key.as_deref())
                            );
                            let _ = web_search_state_sender.send(WebSearchState::Failed {
                                message: message.clone(),
                            });
                            send_chat_notice(
                                &chat_notice_sender,
                                &notice_chat_id,
                                DebugMessage {
                                    message: message.clone(),
                                    is_error: true,
                                },
                            );
                            user_info.chat_history.lock().unwrap().push_message(
                                Correspondence::Bot {
                                    text: format!("Web search failed: {message}"),
                                    model: user_info.model.clone(),
                                    thinking_seconds: None,
                                    sources: Vec::new(),
                                    web_search_used: true,
                                },
                            );
                        }
                    }
                    user_info.chat_history.lock().unwrap().bot_responding = false;
                    return;
                }

                let request: GenerationRequest<'_> =
                    GenerationRequest::new(user_info.model.clone().unwrap(), to_send_prompt)
                        .options(
                            ModelOptions::default()
                                .temperature(user_info.temperature / 10.0)
                                .num_predict(user_info.max_response_tokens as i32)
                                .num_ctx(user_info.context_tokens as u64),
                        )
                        .system(system_prompt.clone());

                println!("System prompt: {}", system_prompt.clone());

                let mut request_body = match serde_json::to_value(request) {
                    Ok(body) => body,
                    Err(e) => {
                        eprintln!("Error serializing request: {}", e);
                        let message = "Could not prepare the Ollama request".to_string();
                        send_chat_notice(
                            &chat_notice_sender,
                            &notice_chat_id,
                            DebugMessage {
                                message: message.clone(),
                                is_error: true,
                            },
                        );
                        append_failed_response(
                            &user_info.chat_history,
                            user_info.model.clone(),
                            message,
                        );
                        user_info.chat_history.lock().unwrap().bot_responding = false;
                        return;
                    }
                };

                request_body["stream"] = serde_json::Value::Bool(true);
                request_body["think"] = user_info.thinking_level.api_value();
                if !attached_images.is_empty() {
                    request_body["images"] = serde_json::json!(
                        attached_images
                            .iter()
                            .map(|image| BASE64.encode(&image.bytes))
                            .collect::<Vec<_>>()
                    );
                }

                let url = format!("http://{}:{}/api/generate", ip.ip, ip.port);
                let client = reqwest::Client::new();
                let response =
                    send_ollama_request_with_retry(&client, &url, &request_body, &cancel).await;
                let response = match response {
                    Ok(Some(response)) => Ok(response),
                    Ok(None) => {
                        user_info.chat_history.lock().unwrap().bot_responding = false;
                        return;
                    }
                    Err(error) => Err(error),
                };
                let mut response = match response {
                    Ok(response) if response.status().is_success() => response,
                    Ok(response) => {
                        let status = response.status();
                        let response_text = response.text();
                        let detail = tokio::select! {
                            detail = response_text => detail.unwrap_or_default(),
                            () = wait_until_cancelled(&cancel) => {
                                user_info.chat_history.lock().unwrap().bot_responding = false;
                                return;
                            }
                        };
                        let message = format!("Ollama rejected the request ({status}): {detail}");
                        send_chat_notice(
                            &chat_notice_sender,
                            &notice_chat_id,
                            DebugMessage {
                                message: message.clone(),
                                is_error: true,
                            },
                        );
                        append_failed_response(
                            &user_info.chat_history,
                            user_info.model.clone(),
                            message,
                        );
                        user_info.chat_history.lock().unwrap().bot_responding = false;
                        return;
                    }
                    Err(error) => {
                        let message = format!("Could not reach Ollama: {error}");
                        send_chat_notice(
                            &chat_notice_sender,
                            &notice_chat_id,
                            DebugMessage {
                                message: message.clone(),
                                is_error: true,
                            },
                        );
                        append_failed_response(
                            &user_info.chat_history,
                            user_info.model.clone(),
                            message,
                        );
                        user_info.chat_history.lock().unwrap().bot_responding = false;
                        return;
                    }
                };

                let mut final_response: Vec<String> = vec![];
                let mut stream_buffer = String::new();

                'response_stream: while !cancel.load(Ordering::Relaxed) {
                    let chunk_result = tokio::select! {
                        chunk = response.chunk() => chunk,
                        _ = tokio::time::sleep(Duration::from_millis(50)) => continue,
                    };
                    let Ok(Some(chunk)) = chunk_result else {
                        break;
                    };
                    stream_buffer.push_str(&String::from_utf8_lossy(&chunk));
                    while let Some(newline) = stream_buffer.find('\n') {
                        let line = stream_buffer[..newline].trim().to_string();
                        stream_buffer.drain(..=newline);
                        if line.is_empty() {
                            continue;
                        }
                        match decode_generation_line(&line) {
                            Ok((mut token, done_reason)) => {
                                if token.done
                                    && (done_reason.as_deref() == Some("length")
                                        || token.eval_count.unwrap_or_default()
                                            >= user_info.max_response_tokens as u64)
                                {
                                    send_chat_notice(
                                        &chat_notice_sender,
                                        &notice_chat_id,
                                        DebugMessage {
                                            message: format!(
                                                "The model reached the generation limit ({} tokens). Increase Maximum response or Context window in Settings.",
                                                user_info.max_response_tokens
                                            ),
                                            is_error: true,
                                        },
                                    );
                                }
                                if !filtering {
                                    print!("{}", token.response);
                                }

                                // Ollama may return reasoning in its dedicated `thinking`
                                // field, while some models emit literal <think> tags.
                                // Normalize both forms so the renderer can disclose them alike.
                                if let Some(thinking) = token.thinking.take()
                                    && !thinking.is_empty()
                                {
                                    token.response =
                                        format!("<think>{thinking}</think>{}", token.response);
                                }

                                final_response.push(token.response.clone());

                                // Filtering must see the complete response: Ollama can
                                // split a profane word across arbitrary stream tokens.
                                if !filtering {
                                    let sent = tokio::select! {
                                        result = tx.send(token) => result.is_ok(),
                                        () = wait_until_cancelled(&cancel) => false,
                                    };
                                    if !sent {
                                        break 'response_stream;
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Error decoding Ollama response: {}", e);
                                send_chat_notice(
                                    &chat_notice_sender,
                                    &notice_chat_id,
                                    DebugMessage {
                                        message: "Ollama returned an invalid streaming response"
                                            .to_string(),
                                        is_error: true,
                                    },
                                );
                            }
                        }
                    }
                }

                let was_cancelled = cancel.load(Ordering::Relaxed);
                // NDJSON normally ends with a newline, but accepting a final
                // unterminated object avoids dropping the last token from
                // proxies or older Ollama builds.
                let trailing_line = stream_buffer.trim();
                if !was_cancelled && !trailing_line.is_empty() {
                    match decode_generation_line(trailing_line) {
                        Ok((mut token, done_reason)) => {
                            if token.done
                                && (done_reason.as_deref() == Some("length")
                                    || token.eval_count.unwrap_or_default()
                                        >= user_info.max_response_tokens as u64)
                            {
                                send_chat_notice(
                                    &chat_notice_sender,
                                    &notice_chat_id,
                                    DebugMessage {
                                        message: format!(
                                            "The model reached the generation limit ({} tokens). Increase Maximum response or Context window in Settings.",
                                            user_info.max_response_tokens
                                        ),
                                        is_error: true,
                                    },
                                );
                            }
                            if let Some(thinking) = token.thinking.take()
                                && !thinking.is_empty()
                            {
                                token.response =
                                    format!("<think>{thinking}</think>{}", token.response);
                            }
                            final_response.push(token.response.clone());
                            if !filtering {
                                let _ = tx.send(token).await;
                            }
                        }
                        Err(error) => {
                            eprintln!("Error decoding final Ollama response: {error}");
                            send_chat_notice(
                                &chat_notice_sender,
                                &notice_chat_id,
                                DebugMessage {
                                    message: "Ollama returned an invalid final streaming response"
                                        .to_string(),
                                    is_error: true,
                                },
                            );
                        }
                    }
                }

                if !was_cancelled && final_response.concat().trim().is_empty() {
                    let message =
                        "Ollama ended the response without returning content.".to_string();
                    send_chat_notice(
                        &chat_notice_sender,
                        &notice_chat_id,
                        DebugMessage {
                            message: message.clone(),
                            is_error: true,
                        },
                    );
                    append_failed_response(
                        &user_info.chat_history,
                        user_info.model.clone(),
                        message,
                    );
                    user_info.chat_history.lock().unwrap().bot_responding = false;
                    return;
                }

                if filtering && !final_response.is_empty() {
                    let filtered = censor_text(&final_response.join(""));
                    final_response = vec![filtered.clone()];
                    let _ = tx
                        .send(GenerationResponse {
                            model: user_info.model.clone().unwrap_or_default(),
                            created_at: Local::now().to_rfc3339(),
                            response: filtered,
                            done: true,
                            context: None,
                            total_duration: None,
                            load_duration: None,
                            prompt_eval_count: None,
                            prompt_eval_duration: None,
                            eval_count: None,
                            eval_duration: None,
                            thinking: None,
                            logprobs: None,
                        })
                        .await;
                }

                if logging && !was_cancelled {
                    Channels::send_request_to_channel(
                        Arc::clone(&channels.logging_channel),
                        Log::create_with_current_time(
                            filtering,
                            user_info.model,
                            final_response.clone(),
                            Some(system_prompt),
                            prompt.clone(),
                        ),
                    );
                }

                if user_info.current_chat_history_enabled && !was_cancelled {
                    let complete = final_response.join("");
                    let (_, visible_response) = split_thinking_text(&complete);
                    user_info
                        .chat_history
                        .lock()
                        .unwrap()
                        .generate_and_push(prompt.clone(), visible_response);
                }

                let partial_response = final_response.join("");
                if !partial_response.is_empty() {
                    let partial_response = disabled_web_tool_message(&partial_response)
                        .unwrap_or(&partial_response)
                        .to_string();
                    user_info
                        .chat_history
                        .lock()
                        .unwrap()
                        .push_message(Correspondence::Bot {
                            text: partial_response,
                            model: None,
                            thinking_seconds: None,
                            sources: Vec::new(),
                            web_search_used: false,
                        });
                }

                user_info.chat_history.lock().unwrap().bot_responding = false;
            },
            move |_| Message::PromptFinished(completion_chat_id.clone()),
        )
    }

    fn boot() -> (Program, Task<Message>) {
        let mut program = Program::default();
        program.refresh_chat_markdown_cache();
        let image_task = program.queue_missing_markdown_images();
        let update_task = program.check_for_updates();
        (program, Task::batch([image_task, update_task]))
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::AsyncResult(_result) => Task::none(),

            Message::PromptFinished(chat_id) => {
                self.finish_prompt(&chat_id);
                self.begin_page_transition();
                self.queue_missing_markdown_images()
            }

            Message::None => Task::none(),

            Message::ToggleImages => {
                self.app_state.gui_state = if self.app_state.gui_state == GUIState::Images {
                    GUIState::Main
                } else {
                    GUIState::Images
                };
                self.begin_page_transition();
                Task::none()
            }

            Message::PickImage => Task::perform(
                async {
                    let paths = rfd::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg", "webp", "gif"])
                        .pick_files()
                        .ok_or_else(|| "No image selected.".to_string())?;
                    paths
                        .iter()
                        .map(|path| load_chat_image(path))
                        .collect::<Result<Vec<_>, _>>()
                },
                Message::ImagesLoaded,
            ),

            Message::DropImage(path) => {
                Task::perform(async move { load_chat_image(&path) }, Message::ImageLoaded)
            }

            Message::PasteImage => {
                Task::perform(async { paste_chat_image() }, Message::ImageLoaded)
            }

            Message::ImageLoaded(result) => {
                match result {
                    Ok(image) => {
                        let name = image.name.clone();
                        self.pending_images.push(image);
                        self.set_debug_message(DebugMessage {
                            message: format!("Attached {name}."),
                            is_error: false,
                        });
                    }
                    Err(error) if error != "No image selected." => {
                        self.set_debug_message(DebugMessage {
                            message: error,
                            is_error: true,
                        })
                    }
                    Err(_) => {}
                }
                Task::none()
            }

            Message::ImagesLoaded(result) => {
                match result {
                    Ok(images) => {
                        let count = images.len();
                        self.pending_images.extend(images);
                        self.set_debug_message(DebugMessage {
                            message: format!("Attached {count} images."),
                            is_error: false,
                        });
                    }
                    Err(error) if error != "No image selected." => {
                        self.set_debug_message(DebugMessage {
                            message: error,
                            is_error: true,
                        })
                    }
                    Err(_) => {}
                }
                Task::none()
            }

            Message::MarkdownImageLoaded { url, result } => {
                self.markdown_images.insert(
                    url,
                    match result {
                        Ok(handle) => MarkdownImageState::Ready(handle),
                        Err(error) => MarkdownImageState::Failed(error),
                    },
                );
                Task::none()
            }

            Message::RemoveImage(index) => {
                if index < self.pending_images.len() {
                    self.pending_images.remove(index);
                }
                Task::none()
            }

            Message::CopyImage(path) => {
                Task::perform(async move { copy_image_file(&path) }, |result| {
                    Message::ImageGenerated(result.map(|_| "__copied__".to_string()))
                })
            }

            Message::GenerateImage => {
                if self.is_generating_image {
                    return Task::none();
                }
                match self.user_information.image_generation_supported {
                    Some(true) => {}
                    Some(false) => {
                        self.set_debug_message(DebugMessage {
                            message:
                                "Choose a model with Ollama's experimental `image` capability."
                                    .to_string(),
                            is_error: true,
                        });
                        return Task::none();
                    }
                    None => {
                        self.set_debug_message(DebugMessage {
                            message: "Waiting for Ollama to report this model's capabilities."
                                .to_string(),
                            is_error: true,
                        });
                        return Task::none();
                    }
                }
                let Some(model) = self.user_information.model.clone() else {
                    self.set_debug_message(DebugMessage {
                        message: "Select an image-generation model first.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                };
                let prompt = self.prompt.prompt.trim().to_string();
                if prompt.is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Describe the image you want to generate.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                self.is_generating_image = true;
                let host = format!(
                    "http://{}:{}",
                    self.user_information.ip_address.ip, self.user_information.ip_address.port
                );
                Task::perform(
                    generate_image_via_ollama(host, model, prompt),
                    Message::ImageGenerated,
                )
            }

            Message::ImageGenerated(result) => {
                self.is_generating_image = false;
                match result {
                    Ok(marker) if marker == "__copied__" => self.set_debug_message(DebugMessage {
                        message: "Image copied to clipboard.".to_string(),
                        is_error: false,
                    }),
                    Ok(path) => {
                        self.generated_images.push(path);
                        self.begin_page_transition();
                        self.set_debug_message(DebugMessage {
                            message: "Image generated locally.".to_string(),
                            is_error: false,
                        });
                    }
                    Err(error) => self.set_debug_message(DebugMessage {
                        message: error,
                        is_error: true,
                    }),
                }
                Task::none()
            }

            Message::ChangeBatchTokens(new_batch_tokens) => {
                self.batch_tokens = new_batch_tokens;
                self.trigger_settings_feedback(SettingsFeedbackTarget::BatchTokens);
                Task::none()
            }

            Message::ToggleFastStreaming => {
                self.fast_streaming = !self.fast_streaming;
                self.persist_boolean_setting("fast_streaming", self.fast_streaming);
                Task::none()
            }

            Message::ToggleChatMenu => {
                self.chat_menu_open = !self.chat_menu_open;
                Task::none()
            }

            Message::StartUiResize(target) => {
                self.ui_resize_target = Some(target);
                Task::none()
            }

            Message::UiResizeMoved(position) => {
                let changed = match self.ui_resize_target {
                    Some(UiResizeTarget::Sidebar) if self.chat_menu_open => {
                        let width = (position.x - 10.0).clamp(MIN_SIDEBAR_WIDTH, MAX_SIDEBAR_WIDTH);
                        if (self.ui_layout.sidebar_width - width).abs() >= 0.5 {
                            self.ui_layout.sidebar_width = width;
                            true
                        } else {
                            false
                        }
                    }
                    Some(UiResizeTarget::Composer) => {
                        let height = (self.window_size.height - position.y - 10.0)
                            .clamp(MIN_COMPOSER_HEIGHT, MAX_COMPOSER_HEIGHT);
                        if (self.ui_layout.composer_height - height).abs() >= 0.5 {
                            self.ui_layout.composer_height = height;
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                };
                if changed {
                    self.persist_ui_layout();
                }
                Task::none()
            }

            Message::StopUiResize => {
                if self.ui_resize_target.take().is_some() {
                    self.persist_ui_layout();
                    self.settings_dirty_at =
                        Some(Instant::now() - Duration::from_millis(SETTINGS_SAVE_DEBOUNCE_MS));
                    self.flush_pending_settings_if_ready();
                }
                Task::none()
            }

            Message::WindowResized(size) => {
                self.window_size = size;
                Task::none()
            }

            Message::ToggleWebSearch => {
                self.web_search_settings.enabled = !self.web_search_settings.enabled;
                if !self.current_chat_is_processing() {
                    self.web_search_for_chat = self.web_search_settings.enabled;
                    self.persist_current_chat_web_search_setting();
                }
                self.persist_web_search_settings();
                Task::none()
            }

            Message::ToggleMultipleWebSearches => {
                self.web_search_settings.allow_multiple_searches =
                    !self.web_search_settings.allow_multiple_searches;
                self.persist_web_search_settings();
                Task::none()
            }

            Message::ToggleDeepResearchControls => {
                self.deep_research_controls_open = !self.deep_research_controls_open;
                self.begin_page_transition();
                Task::none()
            }

            Message::ToggleChatWebSearch => {
                if self.current_chat_is_processing() {
                    self.set_debug_message(DebugMessage {
                        message: "Web search cannot be changed while this chat is working."
                            .to_string(),
                        is_error: false,
                    });
                    return Task::none();
                }
                self.web_search_for_chat = !self.web_search_for_chat;
                self.persist_current_chat_web_search_setting();
                Task::none()
            }

            Message::WebSearchProviderChange(provider) => {
                self.web_search_settings.provider = provider;
                self.persist_web_search_settings();
                Task::none()
            }

            Message::WebSearchApiKeyChange(api_key) => {
                self.web_search_settings
                    .set_selected_api_key(if api_key.is_empty() {
                        None
                    } else {
                        Some(api_key)
                    });
                self.persist_web_search_settings();
                Task::none()
            }

            Message::WebSearchResultLimitChange(value) => {
                self.web_search_settings.result_limit =
                    (value.round() as usize).clamp(1, crate::web_search::MAX_RESULT_LIMIT);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::SearchResultLimit);
                Task::none()
            }

            Message::WebSearchMaximumSearchesChange(value) => {
                self.web_search_settings.maximum_searches =
                    (value.round() as usize).clamp(1, crate::web_search::MAX_CONFIGURABLE_SEARCHES);
                self.web_search_settings.minimum_successful_searches = self
                    .web_search_settings
                    .minimum_successful_searches
                    .min(self.web_search_settings.maximum_searches);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::MaximumSearches);
                Task::none()
            }

            Message::WebSearchMaximumPageFetchesChange(value) => {
                self.web_search_settings.maximum_page_fetches =
                    (value.round() as usize).min(crate::web_search::MAX_CONFIGURABLE_PAGES);
                self.web_search_settings.minimum_independent_pages = self
                    .web_search_settings
                    .minimum_independent_pages
                    .min(self.web_search_settings.maximum_page_fetches);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::MaximumPageReads);
                Task::none()
            }

            Message::WebSearchMinimumSuccessfulSearchesChange(value) => {
                self.web_search_settings.minimum_successful_searches =
                    (value.round() as usize).clamp(1, self.web_search_settings.maximum_searches);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::RequiredSearches);
                Task::none()
            }

            Message::WebSearchMinimumIndependentPagesChange(value) => {
                self.web_search_settings.minimum_independent_pages =
                    (value.round() as usize).min(self.web_search_settings.maximum_page_fetches);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::RequiredPageReads);
                Task::none()
            }

            Message::WebSearchToolIterationLimitChange(value) => {
                self.web_search_settings.tool_iteration_limit = (value.round() as usize)
                    .clamp(2, crate::web_search::MAX_CONFIGURABLE_TOOL_ITERATIONS);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::ToolRounds);
                Task::none()
            }

            Message::WebSearchRequestTimeoutChange(value) => {
                self.web_search_settings.request_timeout_seconds =
                    (value.round() as u64).clamp(3, 60);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::RequestTimeout);
                Task::none()
            }

            Message::WebSearchCustomInstructionsChange(value) => {
                self.web_search_settings.custom_research_instructions = value
                    .chars()
                    .take(crate::web_search::MAX_CUSTOM_RESEARCH_INSTRUCTIONS_CHARS)
                    .collect();
                self.persist_web_search_settings();
                Task::none()
            }

            Message::OpenSource(url) => {
                let parsed = url::Url::parse(&url)
                    .ok()
                    .filter(|url| matches!(url.scheme(), "http" | "https"));
                match parsed {
                    Some(url) => open_url(url.to_string()),
                    None => {
                        self.set_debug_message(DebugMessage {
                            message: "Could not open the source link.".to_string(),
                            is_error: true,
                        });
                        Task::none()
                    }
                }
            }

            Message::CheckForUpdates => self.check_for_updates(),

            Message::AppUpdateChecked(result) => {
                self.app_update_state = match result {
                    Ok(info)
                        if compare_versions(&info.latest_version, APP_VERSION)
                            == Some(ComparisonOrdering::Greater) =>
                    {
                        AppUpdateState::Available {
                            latest_version: info.latest_version,
                            release_url: info.release_url,
                        }
                    }
                    Ok(info) => AppUpdateState::UpToDate {
                        latest_version: info.latest_version,
                    },
                    Err(error) => AppUpdateState::Failed(error),
                };
                Task::none()
            }

            Message::OpenAppUpdate(url) => {
                let trusted_url = url::Url::parse(&url)
                    .ok()
                    .filter(|url| url.scheme() == "https" && url.host_str() == Some("github.com"))
                    .map(|url| url.to_string())
                    .unwrap_or_else(|| UPDATE_RELEASE_PAGE.to_string());
                open_url(trusted_url)
            }

            Message::UrlOpened(result) => {
                if let Err(error) = result {
                    self.set_debug_message(DebugMessage {
                        message: error,
                        is_error: true,
                    });
                }
                Task::none()
            }

            Message::NewChat => {
                self.save_open_chat();
                self.current_chat_id = Self::new_chat_id();
                self.temporary_chat = false;
                self.web_search_for_chat = self.web_search_settings.enabled;
                self.clear_open_chat();
                self.begin_page_transition();
                Task::none()
            }

            Message::OpenChat(id) => self.open_chat(id),

            Message::DeleteChat(id) => {
                if self.active_prompts.contains_key(&id) {
                    self.set_debug_message(DebugMessage {
                        message: "Stop that chat's response before deleting it.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                self.saved_chats.retain(|chat| chat.id != id);
                self.chat_notices.remove(&id);
                self.vision_responses.remove(&id);
                if self.current_chat_id == id {
                    self.current_chat_id = Self::new_chat_id();
                    self.clear_open_chat();
                    self.begin_page_transition();
                }
                self.persist_saved_chats();
                Task::none()
            }

            Message::DeleteTemporaryChat(id) => {
                self.temporary_chats.remove(&id);
                self.chat_notices.remove(&id);
                self.vision_responses.remove(&id);
                if self.current_chat_id == id {
                    self.current_chat_id = Self::new_chat_id();
                    self.temporary_chat = false;
                    self.web_search_for_chat = self.web_search_settings.enabled;
                    self.clear_open_chat();
                    self.begin_page_transition();
                }
                Task::none()
            }

            Message::ToggleChatPin(id) => {
                if let Some(index) = self.saved_chats.iter().position(|chat| chat.id == id) {
                    let mut chat = self.saved_chats.remove(index);
                    chat.pinned = !chat.pinned;
                    let pinned_count =
                        self.saved_chats.iter().filter(|chat| chat.pinned).count();
                    let insert_at = if chat.pinned {
                        // New pins stack right after the chats already pinned.
                        pinned_count
                    } else {
                        // Unpinning returns the chat to its place in time among
                        // the unpinned chats (newest first) instead of leaving
                        // it at the top of the list.
                        self.saved_chats
                            .iter()
                            .skip(pinned_count)
                            .position(|other| other.updated_at < chat.updated_at)
                            .map(|offset| pinned_count + offset)
                            .unwrap_or(self.saved_chats.len())
                    };
                    self.saved_chats.insert(insert_at, chat);
                    self.persist_saved_chats();
                }
                Task::none()
            }

            Message::ToggleTemporaryChat => {
                if self.temporary_chat {
                    if !self.active_prompts.contains_key(&self.current_chat_id) {
                        self.temporary_chats.remove(&self.current_chat_id);
                        self.chat_notices.remove(&self.current_chat_id);
                        self.vision_responses.remove(&self.current_chat_id);
                    }
                    self.temporary_chat = false;
                    self.current_chat_id = Self::new_chat_id();
                    self.web_search_for_chat = self.web_search_settings.enabled;
                    self.clear_open_chat();
                } else {
                    self.save_open_chat();
                    self.temporary_chat = true;
                    self.current_chat_id = Self::new_chat_id();
                    self.web_search_for_chat = self.web_search_settings.enabled;
                    self.clear_open_chat();
                }
                self.begin_page_transition();
                Task::none()
            }

            Message::ChooseChatFolder => {
                if !self.active_prompts.is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Wait for running chats before changing the chat folder."
                            .to_string(),
                        is_error: false,
                    });
                    Task::none()
                } else {
                    Task::perform(
                        async { rfd::FileDialog::new().pick_folder() },
                        Message::ChatFolderSelected,
                    )
                }
            }

            Message::ChatFolderSelected(Some(folder)) => {
                // The folder picker is asynchronous, so a prompt may have
                // started after it opened. Keep the current storage location
                // stable until every background chat has finished.
                if !self.active_prompts.is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Wait for running chats before changing the chat folder."
                            .to_string(),
                        is_error: false,
                    });
                    return Task::none();
                }
                self.save_open_chat();
                let previous_directory = self.chat_storage_dir.clone();
                let result = fs::create_dir_all(&folder)
                    .map_err(|error| error.to_string())
                    .and_then(|_| {
                        self.chat_storage_dir = folder.clone();
                        self.persist_chat_storage_dir()
                    });
                if result.is_ok() {
                    self.saved_chats =
                        read_json_with_backup(&folder.join("chats.json")).unwrap_or_default();
                    // Chats from another install may predate profiles; adopt
                    // them into the legacy profile instead of hiding them.
                    let mut registry = ProfileRegistry {
                        profiles: self.profiles.clone(),
                        active_profile_id: self.active_profile_id.clone(),
                    };
                    let changed = ensure_legacy_profile(&mut registry)
                        | assign_legacy_profile_ids(&mut self.saved_chats);
                    if changed {
                        self.profiles = registry.profiles;
                        self.active_profile_id = registry.active_profile_id;
                        self.persist_profiles();
                        self.persist_saved_chats();
                    }
                } else {
                    self.chat_storage_dir = previous_directory;
                }
                self.set_debug_message(match result {
                    Ok(()) => DebugMessage {
                        message: format!(
                            "Chats will be saved to {}",
                            self.chat_storage_dir.display()
                        ),
                        is_error: false,
                    },
                    Err(error) => DebugMessage {
                        message: format!("Could not use that chat folder: {error}"),
                        is_error: true,
                    },
                });
                Task::none()
            }

            Message::ChatFolderSelected(None) => Task::none(),

            Message::FrameTick => {
                let content_changed = self.drain_live_updates();
                self.advance_ui_motion();
                if content_changed {
                    self.queue_missing_markdown_images()
                } else {
                    Task::none()
                }
            }

            Message::Tick => {
                self.clear_debug_message_if_old();
                self.clear_copy_feedback_if_old();
                self.flush_pending_settings_if_ready();
                let content_changed = self.drain_live_updates();

                if self.current_tick > MAX_TICK {
                    println!("Resetting current tick");
                    self.current_tick = 0;
                }

                self.current_tick += 1;

                if self.current_tick == VERSION_TICK {
                    let ollama_state = Arc::clone(&self.app_state.ollama_state);
                    let user_info = self.user_information.clone();

                    return Task::perform(
                        async move {
                            println!("Checking Ollama version...");
                            let ip = user_info.ip_address;
                            let url = format!("http://{}:{}/api/version", ip.ip, ip.port);

                            match reqwest::get(url).await {
                                Ok(response) => {
                                    println!("API responded with status: {}", response.status());

                                    if response.status().is_success() {
                                        match response.json::<serde_json::Value>().await {
                                            Ok(json) => {
                                                if let Some(version) =
                                                    json.get("version").and_then(|v| v.as_str())
                                                {
                                                    *ollama_state.lock().unwrap() =
                                                        format!("Online (v{})", version);
                                                } else {
                                                    *ollama_state.lock().unwrap() =
                                                        "Online (unknown version)".to_string();
                                                }
                                            }
                                            Err(_) => {
                                                *ollama_state.lock().unwrap() =
                                                    "Online (version parse error)".to_string();
                                            }
                                        }
                                    } else {
                                        *ollama_state.lock().unwrap() = "Offline".to_string();
                                    }
                                }
                                Err(err) => {
                                    println!("Failed to reach API: {}", err);
                                    *ollama_state.lock().unwrap() = "Offline".to_string();
                                }
                            }
                        },
                        Message::AsyncResult,
                    );
                } else if self.current_tick == BOT_LIST_TICK {
                    let ip = self.user_information.ip_address.clone();
                    let ollama = Ollama::builder()
                        .host(format!("http://{}", ip.ip))
                        .port(convert_port_to_u16(ip.port))
                        .build();
                    let bots_list = Arc::clone(&self.app_state.bots_list);
                    let channels = self.channels.clone();

                    return Task::perform(
                        async move {
                            match ollama.list_local_models().await {
                                Ok(bots) => {
                                    let mut names =
                                        bots.into_iter().map(|bot| bot.name).collect::<Vec<_>>();
                                    names.sort();
                                    names.dedup();
                                    *bots_list.lock().unwrap() = names;
                                }
                                Err(e) => {
                                    Channels::send_request_to_channel(
                                        Arc::clone(&channels.debug_channel),
                                        DebugMessage {
                                            message: "Error occurred while listing bots"
                                                .to_string(),
                                            is_error: true,
                                        },
                                    );
                                    bots_list.lock().unwrap().clear();
                                    println!("Error: {:?}", e);
                                }
                            }
                        },
                        Message::AsyncResult,
                    );
                }

                let debug_result = {
                    let guard = self.channels.debug_channel.lock().unwrap();
                    guard.1.try_recv()
                };

                if let Ok(debug_msg) = debug_result {
                    self.set_debug_message(debug_msg);
                }

                let log_result = {
                    let guard = self.channels.logging_channel.lock().unwrap();
                    guard.1.try_recv()
                };

                if let Ok(log) = log_result {
                    self.app_state.logs.push_log(log);

                    let path = history_path();
                    let result = write_json_safely(&path, &self.app_state.logs);
                    match result {
                        Ok(_) => {}
                        Err(_) => {
                            eprintln!("An error writing to history.json");
                            self.set_debug_message(DebugMessage {
                                message: "Failed to write to history.json".to_string(),
                                is_error: true,
                            });
                        }
                    };
                }

                if content_changed {
                    self.queue_missing_markdown_images()
                } else {
                    Task::none()
                }
            }

            Message::ChangeIp(ip) => {
                self.user_information.ip_address.ip = ip;
                Task::none()
            }

            Message::ChangePort(port) => {
                self.user_information.ip_address.port = port;
                Task::none()
            }

            Message::ToggleChatHistory => {
                self.user_information.current_chat_history_enabled =
                    !self.user_information.current_chat_history_enabled;
                self.persist_boolean_setting(
                    "current_chat_history_enabled",
                    self.user_information.current_chat_history_enabled,
                );
                Task::none()
            }

            Message::ToggleFiltering => {
                self.app_state.filtering = !self.app_state.filtering;
                self.app_state.logs.filtering = self.app_state.filtering;
                self.persist_boolean_setting("filtering", self.app_state.filtering);
                Task::none()
            }

            Message::ToggleDarkMode => {
                self.app_state.dark_mode = !self.app_state.dark_mode;
                gui::set_dark_mode(self.app_state.dark_mode);
                self.persist_boolean_setting("dark_mode", self.app_state.dark_mode);
                self.begin_page_transition();
                Task::none()
            }

            Message::WipeChatHistory => {
                // The saved conversation keeps its id and remains on disk. Further
                // messages start a fresh chat, so clearing context cannot overwrite it.
                self.save_open_chat();
                self.current_chat_id = Self::new_chat_id();
                self.clear_open_chat();

                self.set_debug_message(DebugMessage {
                    message: "Current model context cleared. Saved chats were not deleted."
                        .to_string(),
                    is_error: false,
                });
                self.begin_page_transition();

                Task::none()
            }

            Message::UpdateTextSize(n) => {
                self.user_information.text_size = n;
                self.trigger_settings_feedback(SettingsFeedbackTarget::TextSize);
                Task::none()
            }

            Message::ToggleInfoPopup => {
                if self.app_state.gui_state == GUIState::InfoPopup {
                    self.app_state.gui_state = GUIState::Main;
                } else {
                    self.app_state.gui_state = GUIState::InfoPopup;
                }
                self.begin_page_transition();

                Task::none()
            }

            Message::ToggleSettings => {
                if self.app_state.gui_state == GUIState::Settings {
                    self.app_state.gui_state = GUIState::Main;
                } else {
                    self.app_state.gui_state = GUIState::Settings;
                }
                self.begin_page_transition();

                Task::none()
            }

            Message::ToggleAdvancedSettings => {
                if self.app_state.gui_state == GUIState::AdvancedSettings {
                    self.app_state.gui_state = GUIState::Settings;
                } else {
                    self.app_state.gui_state = GUIState::AdvancedSettings;
                }
                self.begin_page_transition();

                Task::none()
            }

            Message::UpdateTemperature(n) => {
                self.user_information.temperature = n;
                self.trigger_settings_feedback(SettingsFeedbackTarget::Temperature);
                Task::none()
            }

            Message::UpdateMaxResponseTokens(value) => {
                let tokens = 1_u32
                    .checked_shl(value.round().clamp(9.0, 20.0) as u32)
                    .unwrap_or(DEFAULT_MAX_RESPONSE_TOKENS)
                    .clamp(MIN_RESPONSE_TOKENS, MAX_RESPONSE_TOKENS);
                self.user_information.max_response_tokens = tokens;
                self.max_response_tokens_input = tokens.to_string();
                self.persist_setting_value("max_response_tokens", serde_json::Value::from(tokens));
                self.trigger_settings_feedback(SettingsFeedbackTarget::MaxResponse);
                Task::none()
            }

            Message::EditMaxResponseTokens(value) => {
                self.max_response_tokens_input = value;
                Task::none()
            }

            Message::ApplyMaxResponseTokens => {
                self.trigger_settings_feedback(SettingsFeedbackTarget::ApplyMaxResponse);
                match self.max_response_tokens_input.trim().parse::<u32>() {
                    Ok(tokens) if (MIN_RESPONSE_TOKENS..=MAX_RESPONSE_TOKENS).contains(&tokens) => {
                        self.user_information.max_response_tokens = tokens;
                        self.persist_setting_value(
                            "max_response_tokens",
                            serde_json::Value::from(tokens),
                        );
                    }
                    _ => self.set_debug_message(DebugMessage {
                        message: format!(
                            "Maximum response must be between {MIN_RESPONSE_TOKENS} and {MAX_RESPONSE_TOKENS} tokens."
                        ),
                        is_error: true,
                    }),
                }
                Task::none()
            }

            Message::UpdateContextTokens(value) => {
                let tokens = 1_u32
                    .checked_shl(value.round().clamp(12.0, 22.0) as u32)
                    .unwrap_or(DEFAULT_CONTEXT_TOKENS)
                    .clamp(MIN_CONTEXT_TOKENS, MAX_CONTEXT_TOKENS);
                self.user_information.context_tokens = tokens;
                self.context_tokens_input = tokens.to_string();
                self.persist_setting_value("context_tokens", serde_json::Value::from(tokens));
                self.trigger_settings_feedback(SettingsFeedbackTarget::ContextWindow);
                Task::none()
            }

            Message::EditContextTokens(value) => {
                self.context_tokens_input = value;
                Task::none()
            }

            Message::ApplyContextTokens => {
                self.trigger_settings_feedback(SettingsFeedbackTarget::ApplyContextWindow);
                match self.context_tokens_input.trim().parse::<u32>() {
                    Ok(tokens) if (MIN_CONTEXT_TOKENS..=MAX_CONTEXT_TOKENS).contains(&tokens) => {
                        self.user_information.context_tokens = tokens;
                        self.persist_setting_value(
                            "context_tokens",
                            serde_json::Value::from(tokens),
                        );
                    }
                    _ => self.set_debug_message(DebugMessage {
                        message: format!(
                            "Context window must be between {MIN_CONTEXT_TOKENS} and {MAX_CONTEXT_TOKENS} tokens."
                        ),
                        is_error: true,
                    }),
                }
                Task::none()
            }

            Message::ToggleCodeChecking => {
                self.code_checking_enabled = !self.code_checking_enabled;
                self.persist_boolean_setting("code_checking_enabled", self.code_checking_enabled);
                Task::none()
            }

            Message::CheckCode(language, code) => {
                if !self.code_checking_enabled {
                    self.set_debug_message(DebugMessage {
                        message:
                            "Enable code checking in Advanced settings before running local tools."
                                .into(),
                        is_error: true,
                    });
                    return Task::none();
                }
                self.set_debug_message(DebugMessage {
                    message: format!("Checking {language} code…"),
                    is_error: false,
                });
                Task::perform(
                    async move { check_code(language, code) },
                    Message::CodeChecked,
                )
            }

            Message::CodeChecked(result) => {
                self.set_debug_message(match result {
                    Ok(message) => DebugMessage {
                        message,
                        is_error: false,
                    },
                    Err(message) => DebugMessage {
                        message,
                        is_error: true,
                    },
                });
                Task::none()
            }

            Message::ToggleDynamicDate => {
                self.dynamic_prompt_settings.include_date =
                    !self.dynamic_prompt_settings.include_date;
                self.persist_dynamic_prompt_settings();
                Task::none()
            }

            Message::ToggleDynamicTime => {
                self.dynamic_prompt_settings.include_time =
                    !self.dynamic_prompt_settings.include_time;
                self.persist_dynamic_prompt_settings();
                Task::none()
            }

            Message::ToggleProfileMenu => {
                self.profile_menu_open = !self.profile_menu_open;
                if !self.profile_menu_open {
                    self.editing_profile_id = None;
                }
                Task::none()
            }

            Message::SelectProfile(id) => {
                self.profile_menu_open = false;
                self.editing_profile_id = None;
                self.switch_profile(id)
            }

            Message::ProfileNameInputChanged(value) => {
                self.profile_name_input = value;
                Task::none()
            }

            Message::CreateProfile => {
                let name = self.profile_name_input.trim().to_string();
                if name.is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Give the new profile a name first.".to_string(),
                        is_error: false,
                    });
                    return Task::none();
                }
                if self
                    .profiles
                    .iter()
                    .any(|profile| profile.name.eq_ignore_ascii_case(&name))
                {
                    self.set_debug_message(DebugMessage {
                        message: "A profile with that name already exists.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                let id = format!(
                    "profile-{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
                );
                self.profiles.push(Profile {
                    id: id.clone(),
                    name: name.clone(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                });
                self.profile_name_input.clear();
                self.profile_menu_open = true;
                // Open the editor so the new profile can be customised right
                // away (user name and instructions stay empty by default).
                self.profile_edit_name = name;
                self.profile_edit_user_name.clear();
                self.profile_edit_instructions.clear();
                self.editing_profile_id = Some(id.clone());
                self.switch_profile(id)
            }

            Message::StartEditProfile(id) => {
                if let Some(profile) = self.profiles.iter().find(|profile| profile.id == id) {
                    self.profile_edit_name = profile.name.clone();
                    self.profile_edit_user_name = profile.user_name.clone();
                    self.profile_edit_instructions = profile.custom_instructions.clone();
                    self.editing_profile_id = Some(id);
                    self.profile_menu_open = true;
                }
                Task::none()
            }

            Message::CancelEditProfile => {
                self.editing_profile_id = None;
                Task::none()
            }

            Message::ProfileEditNameChanged(value) => {
                self.profile_edit_name = value;
                Task::none()
            }

            Message::ProfileEditUserNameChanged(value) => {
                self.profile_edit_user_name = value;
                Task::none()
            }

            Message::ProfileEditInstructionsChanged(value) => {
                self.profile_edit_instructions = value;
                Task::none()
            }

            Message::ConfirmProfileEdits => {
                let Some(id) = self.editing_profile_id.clone() else {
                    return Task::none();
                };
                let name = self.profile_edit_name.trim().to_string();
                if name.is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Profile names cannot be empty.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                if self.profiles.iter().any(|profile| {
                    profile.id != id && profile.name.eq_ignore_ascii_case(&name)
                }) {
                    self.set_debug_message(DebugMessage {
                        message: "A profile with that name already exists.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                if let Some(profile) =
                    self.profiles.iter_mut().find(|profile| profile.id == id)
                {
                    profile.name = name;
                    profile.user_name = self.profile_edit_user_name.trim().to_string();
                    profile.custom_instructions =
                        self.profile_edit_instructions.trim().to_string();
                }
                self.editing_profile_id = None;
                self.persist_profiles();
                Task::none()
            }

            Message::DeleteProfile(id) => {
                if id == self.active_profile_id {
                    self.set_debug_message(DebugMessage {
                        message: "Switch to another profile before deleting this one."
                            .to_string(),
                        is_error: false,
                    });
                    return Task::none();
                }
                let still_has_chats = self
                    .saved_chats
                    .iter()
                    .any(|chat| chat_profile_id(chat) == id)
                    || self
                        .temporary_chats
                        .values()
                        .any(|session| session.profile_id == id)
                    || self
                        .active_prompts
                        .values()
                        .any(|job| job.profile_id == id);
                if still_has_chats {
                    self.set_debug_message(DebugMessage {
                        message: "That profile still has chats. Delete them first."
                            .to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                self.profiles.retain(|profile| profile.id != id);
                if self.editing_profile_id.as_deref() == Some(id.as_str()) {
                    self.editing_profile_id = None;
                }
                self.persist_profiles();
                Task::none()
            }

            Message::DynamicCustomInstructionsChanged(value) => {
                self.dynamic_prompt_settings.custom_instructions = value;
                self.persist_dynamic_prompt_settings();
                Task::none()
            }

            Message::LanguageChange(language) => {
                self.user_information.language = language;
                let value = match language {
                    Language::English => "english",
                    Language::Spanish => "spanish",
                };
                self.persist_setting_value("language", serde_json::Value::String(value.into()));
                Task::none()
            }

            Message::ThinkingLevelChange(level) => {
                self.user_information.thinking_level = level;
                Task::none()
            }

            Message::ToggleThinking(index) => {
                if !self.expanded_thinking.insert(index) {
                    self.expanded_thinking.remove(&index);
                }
                Task::none()
            }

            Message::ToggleSources(index) => {
                if !self.expanded_sources.insert(index) {
                    self.expanded_sources.remove(&index);
                }
                Task::none()
            }

            Message::ModelCapabilitiesKnown(model, capabilities) => {
                if self.user_information.model.as_ref() == Some(&model)
                    && let Some(capabilities) = capabilities
                {
                    let thinking = capabilities
                        .thinking_levels
                        .iter()
                        .any(|level| *level != ThinkingLevel::Off);
                    self.user_information.thinking_supported = Some(thinking);
                    self.user_information.vision_supported = Some(capabilities.vision);
                    self.user_information.image_generation_supported =
                        Some(capabilities.image_generation);
                    self.user_information.thinking_levels = capabilities.thinking_levels;
                    if !self
                        .user_information
                        .thinking_levels
                        .contains(&self.user_information.thinking_level)
                    {
                        self.user_information.thinking_level = self
                            .user_information
                            .thinking_levels
                            .first()
                            .copied()
                            .unwrap_or(ThinkingLevel::Off);
                    }
                }
                Task::none()
            }

            Message::SystemPromptChange(system_prompt) => {
                self.system_prompt.system_prompt = Some(system_prompt);
                Task::none()
            }

            Message::InstallModel(model_install) => {
                Channels::send_request_to_channel(
                    Arc::clone(&self.channels.debug_channel),
                    DebugMessage {
                        message: format!("Installing model... {}", model_install),
                        is_error: false,
                    },
                );

                let ip = self.user_information.ip_address.clone();
                let ollama = Ollama::builder()
                    .host(format!("http://{}", ip.ip))
                    .port(convert_port_to_u16(ip.port))
                    .build();
                let channels = self.channels.clone();

                Task::perform(
                    async move {
                        match ollama.pull_model(model_install.clone(), false).await {
                            Ok(outcome) => {
                                println!(
                                    "Model {} installed successfully: {}",
                                    model_install, outcome.message
                                );
                                Channels::send_request_to_channel(
                                    Arc::clone(&channels.debug_channel),
                                    DebugMessage {
                                        message: format!(
                                            "Installed model {}: {}",
                                            model_install, outcome.message
                                        ),
                                        is_error: false,
                                    },
                                );
                            }
                            Err(outcome) => {
                                println!(
                                    "Failed to install model {}: {:?}",
                                    model_install, outcome
                                );
                                Channels::send_request_to_channel(
                                    Arc::clone(&channels.debug_channel),
                                    DebugMessage {
                                        message: format!(
                                            "Failed to install model {}",
                                            model_install
                                        ),
                                        is_error: true,
                                    },
                                );
                            }
                        };
                    },
                    Message::AsyncResult,
                )
            }

            Message::ModelChange(model) => {
                self.user_information.model = Some(model.clone());
                self.user_information.thinking_supported = None;
                self.user_information.vision_supported = None;
                self.user_information.image_generation_supported = None;
                self.user_information.thinking_levels = vec![ThinkingLevel::Off];
                // Reasoning support and accepted effort values vary by model. Do not carry an
                // effort setting across models while capability detection is still in flight.
                self.user_information.thinking_level = ThinkingLevel::Off;
                let ip = self.user_information.ip_address.clone();
                Task::perform(
                    async move {
                        let url = format!("http://{}:{}/api/show", ip.ip, ip.port);
                        let result = reqwest::Client::new()
                            .post(url)
                            .json(&serde_json::json!({ "model": model }))
                            .send()
                            .await
                            .ok();
                        let capabilities = match result {
                            Some(response) if response.status().is_success() => response
                                .json::<serde_json::Value>()
                                .await
                                .ok()
                                .and_then(|json| model_capabilities(&json)),
                            _ => None,
                        };
                        (model, capabilities)
                    },
                    |(model, capabilities)| Message::ModelCapabilitiesKnown(model, capabilities),
                )
            }

            Message::InstallationPrompt => open_url("https://ollama.com/download".to_string()),

            Message::ListPrompt => open_url("https://ollama.com/search".to_string()),

            Message::CopyPressed(input) => {
                if input.trim().is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Nothing to copy yet.".to_string(),
                        is_error: true,
                    });

                    Task::none()
                } else {
                    self.last_copied_text = Some(input.clone());
                    self.last_copied_at = Some(Instant::now());

                    self.set_debug_message(DebugMessage {
                        message: "Copied to clipboard.".to_string(),
                        is_error: false,
                    });

                    clipboard::write::<Message>(input)
                }
            }

            Message::CopyLatestResponse => {
                let input = self
                    .current_active_prompt()
                    .filter(|job| !job.response_text.trim().is_empty())
                    .map(|job| split_thinking_text(&job.response_text).1)
                    .or_else(|| {
                        self.chat_messages_cache.iter().enumerate().rev().find_map(
                            |(index, message)| {
                                matches!(message, Correspondence::Bot { .. }).then(|| {
                                    self.chat_visible_text_cache
                                        .get(index)
                                        .cloned()
                                        .unwrap_or_default()
                                })
                            },
                        )
                    })
                    .unwrap_or_default();

                if input.trim().is_empty() {
                    self.set_debug_message(DebugMessage {
                        message: "Nothing to copy yet.".to_string(),
                        is_error: true,
                    });
                    Task::none()
                } else {
                    self.last_copied_text = Some(input.clone());
                    self.last_copied_at = Some(Instant::now());
                    self.set_debug_message(DebugMessage {
                        message: "Copied to clipboard.".to_string(),
                        is_error: false,
                    });
                    clipboard::write::<Message>(input)
                }
            }

            Message::KeyPressed(keyboard::Key::Character(key), modifiers)
                if modifiers.control() && key.eq_ignore_ascii_case("v") =>
            {
                Task::perform(async { paste_chat_image() }, Message::ImageLoaded)
            }

            Message::KeyPressed(_, _) => Task::none(),

            Message::KeyReleased(_key) => Task::none(),

            Message::Prompt(prompt) => {
                if !self.current_chat_is_processing() {
                    let mut prompt = prompt.trim().to_string();
                    if prompt.is_empty() && self.pending_images.is_empty() {
                        self.set_debug_message(DebugMessage {
                            message: "Enter a message or attach an image first.".to_string(),
                            is_error: true,
                        });
                        return Task::none();
                    }
                    if !self.pending_images.is_empty()
                        && self.user_information.vision_supported == Some(false)
                    {
                        self.set_debug_message(DebugMessage {
                            message:
                                "The selected model cannot inspect images. Choose a model with the `vision` capability."
                                    .to_string(),
                            is_error: true,
                        });
                        return Task::none();
                    }
                    if self.user_information.model.is_none() {
                        self.set_debug_message(DebugMessage {
                            message: "Select a model before sending a message.".to_string(),
                            is_error: true,
                        });
                        return Task::none();
                    }
                    if prompt.is_empty() {
                        prompt = "Describe this image in detail.".to_string();
                    }
                    self.prompt.prompt.clear();
                    self.prompt.editor = iced::widget::text_editor::Content::new();
                    self.begin_page_transition();
                    return Self::prompt(self, prompt);
                }

                Task::none()
            }

            Message::StopResponse => {
                if let Some(job) = self.active_prompts.get(&self.current_chat_id) {
                    job.cancel.store(true, Ordering::Relaxed);
                }
                self.chat_notices.insert(
                    self.current_chat_id.clone(),
                    (
                        DebugMessage {
                            message: "Stopping response…".to_string(),
                            is_error: false,
                        },
                        Instant::now(),
                    ),
                );
                Task::none()
            }

            Message::EditPrompt(action) => {
                self.prompt.editor.perform(action);
                self.prompt.prompt = self.prompt.editor.text();
                Task::none()
            }

            Message::UseSuggestion(suggestion) => {
                self.prompt.editor = iced::widget::text_editor::Content::with_text(&suggestion);
                self.prompt.prompt = suggestion;
                Task::none()
            }

            Message::UpdateInstall(model) => {
                self.installing_model = model;
                Task::none()
            }
        }
    }

    fn view<'a>(&'a self) -> Element<'a, Message> {
        Self::get_ui_information(self, &self.app_state.gui_state).into()
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![
            iced::event::listen().filter_map(|event| match event {
                iced::event::Event::Window(iced::window::Event::FileDropped(path)) => {
                    Some(Message::DropImage(path))
                }
                iced::event::Event::Window(iced::window::Event::Opened { size, .. })
                | iced::event::Event::Window(iced::window::Event::Resized(size)) => {
                    Some(Message::WindowResized(size))
                }
                iced::event::Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    physical_key,
                    modifiers,
                    ..
                }) if modifiers.command()
                    && (key == keyboard::Key::Character("v".into())
                        || physical_key == keyboard::key::Code::KeyV) =>
                {
                    Some(Message::PasteImage)
                }
                iced::event::Event::Keyboard(keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    ..
                }) => Some(Message::KeyPressed(key, modifiers)),
                iced::event::Event::Keyboard(keyboard::Event::KeyReleased { key, .. }) => {
                    Some(Message::KeyReleased(key))
                }
                _ => None,
            }),
            time::every(Duration::from_millis(TICK_MS)).map(|_| Message::Tick),
        ];
        if self.ui_resize_target.is_some() {
            subscriptions.push(iced::event::listen_with(
                |event, _status, _window| match event {
                    iced::event::Event::Mouse(mouse::Event::CursorMoved { position }) => {
                        Some(Message::UiResizeMoved(position))
                    }
                    iced::event::Event::Mouse(mouse::Event::ButtonReleased(
                        mouse::Button::Left,
                    )) => Some(Message::StopUiResize),
                    _ => None,
                },
            ));
        }
        // Render live content and transitions at display cadence, but stop the
        // frame timer once the interface is idle. Rebuilding the full widget
        // tree continuously wastes CPU and can make later animation stutter.
        if self.needs_frame_updates() {
            subscriptions
                .push(time::every(Duration::from_millis(UI_FRAME_MS)).map(|_| Message::FrameTick));
        }
        Subscription::batch(subscriptions)
    }
}

impl Default for Program {
    fn default() -> Self {
        let mut json_error: String = String::new();

        let prompts_path = resource_path("config/defaultprompts.json");
        let data_prompts: String = match fs::read_to_string(&prompts_path) {
            Ok(dp) => dp,
            Err(_e) => {
                println!("An error occurred reading default prompts");
                json_error.push_str("| Failed to read the installed default prompts");
                "{}".to_string()
            }
        };

        let system_prompts_as_prompt: HashMap<String, String> =
            match serde_json::from_str(&data_prompts) {
                Ok(sp) => sp,
                Err(_e) => {
                    println!("An error occurred reading default prompts (bad format)");
                    json_error.push_str(
                        "| Failed to read: ./config/defaultprompts.json (bad formatting)",
                    );
                    HashMap::from([(String::new(), String::new())])
                }
            };

        let mut system_prompts: Vec<String> = Vec::new();
        system_prompts_as_prompt.iter().for_each(|prompt| {
            system_prompts.push(prompt.0.clone());
        });
        system_prompts.sort();
        let selected_system_prompt = if system_prompts_as_prompt.contains_key("default") {
            Some("default".to_string())
        } else {
            system_prompts.first().cloned()
        };

        println!("Loaded system prompts:\n{:?} ", system_prompts);

        let settings = match load_settings_text() {
            Some(dp) => dp,
            None => {
                println!("An error occurred reading settings");
                json_error.push_str("| Failed to read settings");
                "{}".to_string()
            }
        };

        let settings_hmap: serde_json::Map<String, serde_json::Value> =
            match serde_json::from_str(&settings) {
                Ok(sp) => sp,
                Err(_e) => {
                    println!("An error occurred reading settings (bad format)");
                    json_error.push_str(
                    "| Failed to read: ./config/settings.json (bad formatting. reset to default)",
                );
                    serde_json::Map::new()
                }
            };

        let setting_bool = |key, default| {
            settings_hmap
                .get(key)
                .and_then(|v| v.as_bool())
                .unwrap_or(default)
        };
        let setting_u32 = |key, default, minimum, maximum| {
            settings_hmap
                .get(key)
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .unwrap_or(default)
                .clamp(minimum, maximum)
        };
        let filtering = setting_bool("filtering", true);
        let dark_mode = setting_bool("dark_mode", true);
        let logging = setting_bool("logging", false);
        let info_popup = setting_bool("info_popup", false);
        let fast_streaming = setting_bool("fast_streaming", true);
        let current_chat_history_enabled = setting_bool("current_chat_history_enabled", true);
        let code_checking_enabled = setting_bool("code_checking_enabled", false);
        let dynamic_prompt_settings = settings_hmap
            .get("dynamic_prompt")
            .cloned()
            .and_then(|value| serde_json::from_value::<DynamicPromptSettings>(value).ok())
            .unwrap_or_default();
        let web_search_settings = settings_hmap
            .get("web_search")
            .cloned()
            .and_then(|value| serde_json::from_value::<WebSearchSettings>(value).ok())
            .unwrap_or_default()
            .normalized();
        let ui_layout = settings_hmap
            .get("ui_layout")
            .cloned()
            .and_then(|value| serde_json::from_value::<UiLayoutSettings>(value).ok())
            .unwrap_or_default()
            .normalized();
        let max_response_tokens = setting_u32(
            "max_response_tokens",
            DEFAULT_MAX_RESPONSE_TOKENS,
            MIN_RESPONSE_TOKENS,
            MAX_RESPONSE_TOKENS,
        );
        let context_tokens = setting_u32(
            "context_tokens",
            DEFAULT_CONTEXT_TOKENS,
            MIN_CONTEXT_TOKENS,
            MAX_CONTEXT_TOKENS,
        );
        let language = match settings_hmap
            .get("language")
            .and_then(|value| value.as_str())
        {
            Some("spanish") => Language::Spanish,
            _ => Language::English,
        };
        let legacy_configured_dir = settings_hmap
            .get("chat_storage_dir")
            .and_then(|value| value.as_str())
            .filter(|path| !path.trim().is_empty())
            .map(PathBuf::from);
        let chat_storage_dir =
            read_json_with_backup::<serde_json::Value>(&chat_location_settings_path())
                .and_then(|value| {
                    value
                        .get("chat_storage_dir")
                        .and_then(|path| path.as_str())
                        .map(PathBuf::from)
                })
                .or(legacy_configured_dir)
                .unwrap_or_else(default_chat_storage_dir);

        let mut saved_chats: Vec<SavedChat> =
            read_json_with_backup(&chat_storage_dir.join("chats.json"))
                .or_else(|| {
                    fs::read_to_string("./output/chats.json")
                        .ok()
                        .and_then(|data| serde_json::from_str(&data).ok())
                })
                .unwrap_or_default();

        // First launch after the profiles update: adopt every existing chat
        // into the legacy "Outdated Profile" so nothing is lost or moved.
        // The registry file is written the first time the user changes it.
        let mut profiles_registry =
            read_json_with_backup::<ProfileRegistry>(&profiles_path()).unwrap_or_default();
        ensure_legacy_profile(&mut profiles_registry);
        assign_legacy_profile_ids(&mut saved_chats);
        let profiles = profiles_registry.profiles.clone();
        let active_profile_id = profiles_registry.active_profile_id.clone();

        let restored_chat = saved_chats
            .iter()
            .filter(|chat| chat_profile_id(chat) == active_profile_id)
            .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
            .cloned();
        let current_chat_id = restored_chat
            .as_ref()
            .map(|chat| chat.id.clone())
            .unwrap_or_else(Self::new_chat_id);
        let web_search_for_chat = restored_chat
            .as_ref()
            .and_then(|chat| chat.web_search_enabled)
            .unwrap_or(web_search_settings.enabled);
        let current_chat =
            restored_chat
                .as_ref()
                .map(SavedChat::to_current)
                .unwrap_or(CurrentChat {
                    chats: vec![],
                    messages: vec![],
                    bot_responding: false,
                });

        let history_file = history_path();
        let mut history = read_json_with_backup::<History>(&history_file).unwrap_or(History {
            began_logging: Local::now().to_rfc3339(),
            version: APP_VERSION.to_string(),
            filtering,
            logs: vec![],
        });
        history.version = APP_VERSION.to_string();
        history.filtering = filtering;

        let (chat_notice_sender, chat_notice_receiver) = crossbeam_channel::unbounded();
        gui::set_dark_mode(dark_mode);

        Self {
            batch_tokens: 3,
            fast_streaming,
            chat_menu_open: true,
            sidebar_animation: 1.0,
            ui_motion: 0.0,
            page_reveal: 0.0,
            ui_layout,
            ui_resize_target: None,
            window_size: Size::new(1100.0, 800.0),
            temporary_chat: false,
            web_search_for_chat,
            web_search_settings,
            current_chat_id,
            open_chat_dirty: false,
            saved_chats,
            chat_storage_dir,
            profiles,
            active_profile_id,
            profile_menu_open: false,
            profile_name_input: String::new(),
            editing_profile_id: None,
            profile_edit_name: String::new(),
            profile_edit_user_name: String::new(),
            profile_edit_instructions: String::new(),
            code_checking_enabled,
            dynamic_prompt_settings,
            max_response_tokens_input: max_response_tokens.to_string(),
            context_tokens_input: context_tokens.to_string(),
            pending_settings: serde_json::Map::new(),
            settings_dirty_at: None,
            active_prompts: HashMap::new(),
            temporary_chats: HashMap::new(),
            chat_notices: HashMap::new(),
            chat_notice_sender,
            chat_notice_receiver,
            current_tick: 0,
            installing_model: String::new(),
            app_update_state: AppUpdateState::Idle,

            debug_message: DebugMessage {
                message: json_error.clone(),
                is_error: json_error != String::new(),
            },
            debug_message_set_at: if json_error.is_empty() {
                None
            } else {
                Some(Instant::now())
            },
            chat_markdown_cache: Vec::new(),
            chat_messages_cache: Vec::new(),
            chat_thinking_cache: Vec::new(),
            chat_visible_text_cache: Vec::new(),
            chat_model_name_cache: Vec::new(),
            last_copied_text: None,
            last_copied_at: None,
            pending_images: Vec::new(),
            generated_images: load_generated_images(),
            is_generating_image: false,
            vision_responses: HashMap::new(),
            markdown_images: HashMap::new(),
            expanded_thinking: HashSet::new(),
            expanded_sources: HashSet::new(),
            deep_research_controls_open: false,
            settings_feedback: None,
            brand_icon: iced::widget::image::Handle::from_bytes(
                include_bytes!("../assets/icon-transparent.png").to_vec(),
            ),

            system_prompt: SystemPrompt {
                system_prompts_as_hashmap: system_prompts_as_prompt,
                system_prompts_as_vec: Arc::new(Mutex::new(system_prompts)),
                system_prompt: selected_system_prompt,
            },
            channels: Channels {
                debug_channel: Arc::new(Mutex::new(std::sync::mpsc::channel::<DebugMessage>())),
                logging_channel: Arc::new(Mutex::new(std::sync::mpsc::channel::<Log>())),
            },
            user_information: UserInformation {
                chat_history: Arc::new(Mutex::new(current_chat)),
                current_chat_history_enabled,
                model: None,
                thinking_level: ThinkingLevel::Off,
                thinking_levels: vec![ThinkingLevel::Off],
                thinking_supported: None,
                vision_supported: None,
                image_generation_supported: None,
                max_response_tokens,
                context_tokens,
                temperature: 7.0,
                text_size: 24.0,
                ip_address: HostLocation {
                    ip: "127.0.0.1".to_string(),
                    port: "11434".to_string(),
                },
                language,
            },
            prompt: Prompt {
                prompt: String::new(),
                editor: iced::widget::text_editor::Content::new(),
            },
            app_state: AppState {
                filtering,
                dark_mode,
                gui_state: if info_popup {
                    GUIState::InfoPopup
                } else {
                    GUIState::Main
                },
                logs: history,
                logging,
                ollama_state: Arc::new(Mutex::new("Offline".to_string())),
                bots_list: Arc::new(Mutex::new(vec![])),
            },
        }
    }
}

pub fn main() -> iced::Result {
    // Keep the live window icon in the executable. In particular, Linux launchers
    // do not guarantee a working directory beside the installed asset folder.
    let icon = image::load_from_memory_with_format(
        include_bytes!("../assets/icon.png"),
        image::ImageFormat::Png,
    )
    .map_err(|error| error.to_string())
    .and_then(|image| {
        let rgba_image = image.into_rgba8();
        let (width, height) = rgba_image.dimensions();
        iced::window::icon::from_rgba(rgba_image.into_raw(), width, height)
            .map_err(|error| error.to_string())
    })
    .map(Some)
    .unwrap_or_else(|error| {
        eprintln!("Failed to load the embedded application icon: {error}");
        None
    });

    let window_settings = iced::window::Settings {
        icon,
        min_size: Some(Size::new(900.0, 640.0)),
        ..iced::window::Settings::default()
    };

    // Wayland associates a running window with its .desktop file through this
    // ID. Matching the installed desktop-file basename lets docks display the
    // application icon instead of a generic placeholder.
    #[cfg(target_os = "linux")]
    let window_settings = iced::window::Settings {
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: LINUX_APP_ID.to_string(),
            ..iced::window::settings::PlatformSpecific::default()
        },
        ..window_settings
    };

    let application = iced::application(Program::boot, Program::update, Program::view)
        .subscription(Program::subscription)
        .theme(|program: &Program| {
            if program.app_state.dark_mode {
                Theme::Dark
            } else {
                Theme::Light
            }
        })
        .window_size(Size::new(1100.0, 800.0))
        .window(window_settings)
        .antialiasing(true);

    fallback_font_bytes()
        .into_iter()
        .fold(application, |application, font| application.font(font))
        .run()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Instant, SystemTime, UNIX_EPOCH};

    use iced_widget::markdown;

    use super::{
        ActivePrompt, Correspondence, CurrentChat, LEGACY_PROFILE_ID, Message, ModelCapabilities,
        Point, Profile, ProfileRegistry, Program, SavedChat, SettingsFeedbackTarget, Size,
        ThinkingLevel, ToolLoopProgress, UiResizeTarget, UserInformation, WebSearchState,
        app_data_dir, assign_legacy_profile_ids, canonical_code_language, censor_text,
        chat_profile_id, compare_versions, conversation_context_prompt,
        decode_generation_line, disabled_web_tool_message, ensure_legacy_profile,
        generated_image_payload, model_capabilities, normalize_code_fence_languages,
        parse_markdown_items, read_json_with_backup, remote_image_url_is_safe, sidecar_path,
        split_thinking_text, write_json_safely,
    };

    fn test_active_prompt(
        chat_history: Arc<Mutex<CurrentChat>>,
        cancel: Arc<AtomicBool>,
    ) -> ActivePrompt {
        let (_render_sender, render_receiver) = crossbeam_channel::unbounded();
        let (_web_sender, web_search_state_receiver) = crossbeam_channel::unbounded();
        let (_progress_sender, web_progress_receiver) =
            tokio::sync::watch::channel(ToolLoopProgress::default());
        ActivePrompt {
            chat_history,
            response_text: "Background answer".to_string(),
            thinking_text: String::new(),
            parsed_markdown: Vec::new(),
            render_receiver,
            web_search_state: WebSearchState::Idle,
            web_search_state_receiver,
            web_progress_receiver,
            cancel,
            model_name: "test-model".to_string(),
            started_at: Instant::now(),
            response_start_index: 1,
            had_image: false,
            web_search_enabled: true,
            temporary: false,
            profile_id: LEGACY_PROFILE_ID.to_string(),
        }
    }

    fn empty_current_chat() -> CurrentChat {
        CurrentChat {
            chats: vec![],
            messages: vec![],
            bot_responding: false,
        }
    }

    fn test_saved_chat(id: &str, profile: &str, updated_at: &str) -> SavedChat {
        let mut chat = SavedChat::from_current(
            id.to_string(),
            format!("{id} title"),
            &empty_current_chat(),
            false,
            profile.to_string(),
        );
        chat.updated_at = updated_at.to_string();
        chat
    }

    #[test]
    fn content_filter_replaces_entire_inappropriate_words_with_hashes() {
        assert_eq!(censor_text("hello crap"), "hello ####");
    }

    #[test]
    fn update_versions_compare_numerically_and_accept_v_prefixes() {
        assert_eq!(
            compare_versions("v0.10.0", "0.9.9"),
            Some(std::cmp::Ordering::Greater)
        );
        assert_eq!(
            compare_versions("0.6", "v0.6.0"),
            Some(std::cmp::Ordering::Equal)
        );
        assert_eq!(compare_versions("not-a-version", "0.6.0"), None);
    }

    #[test]
    fn prompt_editor_preserves_multiple_lines() {
        use iced::widget::text_editor::{Action, Edit};

        let mut program = Program::default();
        let _ = program.update(Message::EditPrompt(Action::Edit(Edit::Paste(Arc::new(
            "first line".to_string(),
        )))));
        let _ = program.update(Message::EditPrompt(Action::Edit(Edit::Enter)));
        let _ = program.update(Message::EditPrompt(Action::Edit(Edit::Paste(Arc::new(
            "second line".to_string(),
        )))));

        assert_eq!(program.prompt.prompt, "first line\nsecond line");
    }

    #[test]
    fn starter_suggestion_fills_the_composer_without_sending() {
        let mut program = Program::default();
        let suggestion = "Help me plan a small project.".to_string();

        let _ = program.update(Message::UseSuggestion(suggestion.clone()));

        assert_eq!(program.prompt.prompt, suggestion);
        assert_eq!(program.prompt.editor.text(), suggestion);
        assert!(program.active_prompts.is_empty());
    }

    #[test]
    fn sidebar_motion_eases_toward_its_target() {
        let mut program = Program {
            chat_menu_open: false,
            ..Program::default()
        };
        program.advance_ui_motion();
        assert!(program.sidebar_animation < 1.0);
        assert!(program.sidebar_animation > 0.0);

        for _ in 0..90 {
            program.advance_ui_motion();
        }
        assert_eq!(program.sidebar_animation, 0.0);

        program.chat_menu_open = true;
        program.advance_ui_motion();
        assert!(program.sidebar_animation > 0.0);
    }

    #[test]
    fn frame_timer_stops_after_idle_animations_finish() {
        let mut program = Program {
            page_reveal: 1.0,
            sidebar_animation: 1.0,
            ..Program::default()
        };
        assert!(!program.needs_frame_updates());

        program.begin_page_transition();
        assert!(program.needs_frame_updates());
    }

    #[test]
    fn response_sources_are_collapsed_until_explicitly_opened() {
        let mut program = Program::default();
        assert!(program.expanded_sources.is_empty());

        let _ = program.update(Message::ToggleSources(3));
        assert!(program.expanded_sources.contains(&3));

        let _ = program.update(Message::ToggleSources(3));
        assert!(!program.expanded_sources.contains(&3));
    }

    #[test]
    fn settings_value_changes_start_spring_feedback_frames() {
        let mut program = Program {
            page_reveal: 1.0,
            sidebar_animation: 1.0,
            ..Program::default()
        };
        assert!(program.settings_feedback.is_none());

        let _ = program.update(Message::UpdateTemperature(0.7));
        assert!(matches!(
            program.settings_feedback,
            Some((SettingsFeedbackTarget::Temperature, _))
        ));
        assert!(program.needs_frame_updates());

        let _ = program.update(Message::ApplyContextTokens);
        assert!(matches!(
            program.settings_feedback,
            Some((SettingsFeedbackTarget::ApplyContextWindow, _))
        ));
    }

    #[test]
    fn deep_research_controls_are_collapsed_by_default() {
        let mut program = Program::default();
        assert!(!program.deep_research_controls_open);

        let _ = program.update(Message::ToggleDeepResearchControls);
        assert!(program.deep_research_controls_open);
    }

    #[test]
    fn safe_json_writes_retain_a_parseable_backup() {
        let directory = std::env::temp_dir().join(format!(
            "locoryn-safe-write-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = directory.join("settings.json");
        write_json_safely(&path, &serde_json::json!({"revision": 1})).unwrap();
        write_json_safely(&path, &serde_json::json!({"revision": 2})).unwrap();

        std::fs::write(&path, b"incomplete").unwrap();
        let restored: serde_json::Value = read_json_with_backup(&path).unwrap();
        assert_eq!(restored["revision"], 1);
        assert!(sidecar_path(&path, ".bak").exists());

        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn finished_chat_render_cache_is_reused_until_messages_change() {
        let mut program = Program::default();
        *program.user_information.chat_history.lock().unwrap() = CurrentChat {
            chats: Vec::new(),
            messages: vec![Correspondence::Bot {
                text: "<think>Reasoning</think>Visible answer".into(),
                model: Some("test-model".into()),
                thinking_seconds: Some(1),
                sources: Vec::new(),
                web_search_used: false,
            }],
            bot_responding: false,
        };

        program.refresh_chat_markdown_cache();
        let markdown_address = program.chat_markdown_cache[0].as_ptr();
        assert_eq!(program.chat_thinking_cache[0], "Reasoning");
        assert_eq!(program.chat_visible_text_cache[0], "Visible answer");

        program.refresh_chat_markdown_cache();
        assert_eq!(program.chat_markdown_cache[0].as_ptr(), markdown_address);
        assert_eq!(program.chat_messages_cache.len(), 1);
    }

    #[test]
    fn web_progress_stream_updates_answer_and_opens_live_thinking() {
        let mut program = Program::default();
        let chat_id = program.current_chat_id.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut prompt =
            test_active_prompt(Arc::clone(&program.user_information.chat_history), cancel);
        let (progress_sender, progress_receiver) =
            tokio::sync::watch::channel(ToolLoopProgress::default());
        prompt.web_progress_receiver = progress_receiver;
        program.active_prompts.insert(chat_id, prompt);

        progress_sender.send_replace(ToolLoopProgress {
            thinking: "Comparing the retrieved sources.".to_string(),
            answer: "The streamed answer has started.".to_string(),
        });
        program.drain_live_updates();

        let active = program.current_active_prompt().unwrap();
        assert_eq!(active.thinking_text, "Comparing the retrieved sources.");
        assert!(
            active
                .response_text
                .contains("The streamed answer has started.")
        );
        assert!(!active.parsed_markdown.is_empty());
        assert!(program.expanded_thinking.contains(&usize::MAX));
    }

    #[test]
    fn rapid_setting_changes_are_coalesced_before_disk_io() {
        let mut program = Program::default();
        program.persist_setting_value("test-setting", serde_json::json!(1));
        program.persist_setting_value("test-setting", serde_json::json!(2));

        assert_eq!(program.pending_settings.len(), 1);
        assert_eq!(
            program.pending_settings.get("test-setting"),
            Some(&serde_json::json!(2))
        );
        assert!(program.settings_dirty_at.is_some());
    }

    #[test]
    fn drag_resizing_updates_bounded_persisted_layout_values() {
        let mut program = Program {
            window_size: Size::new(1100.0, 800.0),
            ..Program::default()
        };

        let _ = program.update(Message::StartUiResize(UiResizeTarget::Sidebar));
        let _ = program.update(Message::UiResizeMoved(Point::new(390.0, 300.0)));
        assert_eq!(program.ui_layout.sidebar_width, 380.0);

        let _ = program.update(Message::StartUiResize(UiResizeTarget::Composer));
        let _ = program.update(Message::UiResizeMoved(Point::new(500.0, 590.0)));
        assert_eq!(program.ui_layout.composer_height, 200.0);
        assert!(program.pending_settings.contains_key("ui_layout"));
    }

    #[test]
    fn code_checker_recognizes_only_supported_fence_languages() {
        assert_eq!(canonical_code_language("python3"), Some("Python"));
        assert_eq!(canonical_code_language("rs"), Some("Rust"));
        assert_eq!(canonical_code_language("c++"), Some("C++"));
        assert_eq!(canonical_code_language("csharp"), Some("C#"));
        assert_eq!(canonical_code_language("javascript"), None);
    }

    #[test]
    fn separates_thinking_from_answer() {
        assert_eq!(
            split_thinking_text("<think>work it out</think>The answer."),
            ("work it out".into(), "The answer.".into())
        );
    }

    #[test]
    fn combines_streamed_thinking_blocks() {
        assert_eq!(
            split_thinking_text("<think>first </think><think>second</think>Done"),
            ("first second".into(), "Done".into())
        );
    }

    #[test]
    fn hides_unclosed_streaming_thinking() {
        assert_eq!(
            split_thinking_text("Intro<think>still reasoning"),
            ("still reasoning".into(), "Intro".into())
        );
    }

    #[test]
    fn reads_distinct_ollama_image_capabilities() {
        let details = serde_json::json!({
            "capabilities": ["completion", "thinking", "vision"]
        });
        assert_eq!(
            model_capabilities(&details),
            Some(ModelCapabilities {
                thinking_levels: vec![ThinkingLevel::Off, ThinkingLevel::On],
                vision: true,
                image_generation: false,
            })
        );

        let generator = serde_json::json!({ "capabilities": ["image"] });
        assert_eq!(
            model_capabilities(&generator),
            Some(ModelCapabilities {
                thinking_levels: vec![ThinkingLevel::Off],
                vision: false,
                image_generation: true,
            })
        );
    }

    #[test]
    fn detects_model_specific_reasoning_efforts() {
        let reported = serde_json::json!({
            "capabilities": ["completion", "thinking"],
            "model_info": {
                "supported_reasoning_levels": ["none", "minimal", "low", "high", "max"]
            }
        });
        assert_eq!(
            model_capabilities(&reported).unwrap().thinking_levels,
            vec![
                ThinkingLevel::Off,
                ThinkingLevel::Minimal,
                ThinkingLevel::Low,
                ThinkingLevel::High,
                ThinkingLevel::Max,
            ]
        );

        let harmony = serde_json::json!({
            "capabilities": ["completion", "thinking"],
            "renderer": "harmony"
        });
        assert_eq!(
            model_capabilities(&harmony).unwrap().thinking_levels,
            vec![
                ThinkingLevel::Low,
                ThinkingLevel::Medium,
                ThinkingLevel::High,
            ]
        );
    }

    #[test]
    fn retains_ollama_generation_stop_reason() {
        let line = r#"{
            "model":"test",
            "created_at":"2026-01-01T00:00:00Z",
            "response":"",
            "done":true,
            "done_reason":"length",
            "eval_count":10240
        }"#;
        let (response, reason) = decode_generation_line(line).unwrap();
        assert!(response.done);
        assert_eq!(reason.as_deref(), Some("length"));
    }

    #[test]
    fn explains_disabled_model_web_tool_attempts() {
        assert!(
            disabled_web_tool_message(r#"{"tool":"web_search","arguments":{"query":"today"}}"#)
                .is_some()
        );
        assert!(disabled_web_tool_message("A normal answer about web search.").is_none());
    }

    #[test]
    fn failed_prompt_does_not_relabel_an_older_response() {
        let mut chat = CurrentChat {
            chats: Vec::new(),
            messages: vec![Correspondence::Bot {
                text: "Older answer".into(),
                model: None,
                thinking_seconds: None,
                sources: Vec::new(),
                web_search_used: false,
            }],
            bot_responding: false,
        };

        Program::apply_response_metadata(&mut chat, 1, "new-model", 9);

        assert!(matches!(
            &chat.messages[0],
            Correspondence::Bot {
                model: None,
                thinking_seconds: None,
                ..
            }
        ));
    }

    #[test]
    fn response_metadata_only_updates_the_new_prompt_boundary() {
        let mut chat = CurrentChat {
            chats: Vec::new(),
            messages: vec![
                Correspondence::Bot {
                    text: "Older answer".into(),
                    model: None,
                    thinking_seconds: None,
                    sources: Vec::new(),
                    web_search_used: false,
                },
                Correspondence::User {
                    text: "New question".into(),
                    images: Vec::new(),
                },
                Correspondence::Bot {
                    text: "New answer".into(),
                    model: None,
                    thinking_seconds: None,
                    sources: Vec::new(),
                    web_search_used: false,
                },
            ],
            bot_responding: false,
        };

        Program::apply_response_metadata(&mut chat, 2, "new-model", 9);

        assert!(matches!(
            &chat.messages[0],
            Correspondence::Bot {
                model: None,
                thinking_seconds: None,
                ..
            }
        ));
        assert!(matches!(
            &chat.messages[2],
            Correspondence::Bot {
                model: Some(model),
                thinking_seconds: Some(9),
                ..
            } if model == "new-model"
        ));
    }

    #[test]
    fn navigating_away_keeps_prompt_running_and_finishes_its_original_chat() {
        let test_app_data_dir = app_data_dir();
        let _ = std::fs::remove_dir_all(&test_app_data_dir);
        let mut program = Program::default();
        program.active_prompts.clear();
        program.saved_chats.clear();
        program.temporary_chats.clear();
        let test_storage_dir = std::env::temp_dir().join(format!(
            "locoryn-background-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        program.chat_storage_dir.clone_from(&test_storage_dir);

        let chat_a_id = "background-chat-a".to_string();
        let chat_a = Arc::new(Mutex::new(CurrentChat {
            chats: Vec::new(),
            messages: vec![Correspondence::User {
                text: "Background question".to_string(),
                images: Vec::new(),
            }],
            bot_responding: true,
        }));
        let cancel = Arc::new(AtomicBool::new(false));

        program.current_chat_id.clone_from(&chat_a_id);
        program.user_information.chat_history = Arc::clone(&chat_a);
        program.active_prompts.insert(
            chat_a_id.clone(),
            test_active_prompt(Arc::clone(&chat_a), Arc::clone(&cancel)),
        );

        let original_storage_dir = program.chat_storage_dir.clone();
        drop(program.update(Message::ChatFolderSelected(Some(
            original_storage_dir.join("should-not-be-used"),
        ))));
        assert_eq!(program.chat_storage_dir, original_storage_dir);

        drop(program.update(Message::NewChat));
        let foreground_chat_id = program.current_chat_id.clone();
        assert_ne!(foreground_chat_id, chat_a_id);
        assert!(!cancel.load(Ordering::Relaxed));
        assert!(program.active_prompts.contains_key(&chat_a_id));

        chat_a.lock().unwrap().push_message(Correspondence::Bot {
            text: "Background answer".to_string(),
            model: None,
            thinking_seconds: None,
            sources: Vec::new(),
            web_search_used: true,
        });
        chat_a.lock().unwrap().bot_responding = false;

        drop(program.update(Message::PromptFinished(chat_a_id.clone())));

        assert!(!program.active_prompts.contains_key(&chat_a_id));
        assert_eq!(program.current_chat_id, foreground_chat_id);
        assert!(
            program
                .user_information
                .chat_history
                .lock()
                .unwrap()
                .messages
                .is_empty()
        );
        let saved_chat = program
            .saved_chats
            .iter()
            .find(|chat| chat.id == chat_a_id)
            .expect("background chat should be updated in saved chats");
        assert_eq!(saved_chat.messages.len(), 2);
        assert_eq!(saved_chat.web_search_enabled, Some(true));
        let reopened_chat = saved_chat.to_current();
        assert!(matches!(
            &reopened_chat.messages[1],
            Correspondence::Bot {
                text,
                model: Some(model),
                ..
            } if text == "Background answer" && model == "test-model"
        ));

        drop(program.update(Message::OpenChat(chat_a_id.clone())));
        assert!(program.web_search_for_chat);
        drop(program.update(Message::ToggleChatWebSearch));
        assert!(!program.web_search_for_chat);
        assert_eq!(
            program
                .saved_chats
                .iter()
                .find(|chat| chat.id == chat_a_id)
                .and_then(|chat| chat.web_search_enabled),
            Some(false)
        );

        std::fs::remove_dir_all(test_storage_dir).unwrap();
        if test_app_data_dir.exists() {
            std::fs::remove_dir_all(test_app_data_dir).unwrap();
        }
    }

    #[test]
    fn normalizes_common_code_fence_language_aliases_without_touching_code() {
        let input = "```csharp\nlet marker = \"```csharp\";\n```\n~~~cplusplus\nint main() {}\n~~~";
        let normalized = normalize_code_fence_languages(input);
        assert_eq!(
            normalized,
            "```cs\nlet marker = \"```csharp\";\n```\n~~~cpp\nint main() {}\n~~~"
        );

        let items = parse_markdown_items("```csharp\nConsole.WriteLine(\"hi\");\n```");
        assert!(matches!(
            &items[0],
            markdown::Item::CodeBlock {
                language: Some(language),
                ..
            } if language == "cs"
        ));
    }

    #[test]
    fn reads_current_and_legacy_generated_image_payloads() {
        let current = serde_json::json!({
            "data": [{ "b64_json": "current-image" }]
        });
        let legacy = serde_json::json!({ "image": "legacy-image" });

        assert_eq!(generated_image_payload(&current), Some("current-image"));
        assert_eq!(generated_image_payload(&legacy), Some("legacy-image"));
        assert_eq!(generated_image_payload(&serde_json::json!({})), None);
    }

    #[test]
    fn markdown_images_block_local_network_addresses() {
        assert!(remote_image_url_is_safe(
            &url::Url::parse("https://images.example.com/cat.png").unwrap()
        ));
        for blocked in [
            "http://localhost/image.png",
            "http://127.0.0.1/image.png",
            "http://169.254.10.2/image.png",
            "http://[::1]/image.png",
            "file:///tmp/private.png",
        ] {
            assert!(
                !remote_image_url_is_safe(&url::Url::parse(blocked).unwrap()),
                "{blocked} should be blocked"
            );
        }
    }

    #[test]
    fn pre_profile_chats_are_adopted_by_the_outdated_profile() {
        let legacy_json = r#"{
            "id":"chat-old",
            "title":"Older chat",
            "updated_at":"2026-01-01T00:00:00Z",
            "context":[],
            "messages":[]
        }"#;
        let mut chats: Vec<SavedChat> = vec![serde_json::from_str(legacy_json).unwrap()];
        assert_eq!(chats[0].profile, None);

        let changed = assign_legacy_profile_ids(&mut chats);

        assert!(changed);
        assert_eq!(chats[0].profile.as_deref(), Some(LEGACY_PROFILE_ID));
        assert_eq!(chat_profile_id(&chats[0]), LEGACY_PROFILE_ID);
    }

    #[test]
    fn profiled_chats_keep_their_profile_during_migration() {
        let mut chats = vec![test_saved_chat("chat-1", "profile-9", "2026-01-01T00:00:00Z")];

        let changed = assign_legacy_profile_ids(&mut chats);

        assert!(!changed);
        assert_eq!(chats[0].profile.as_deref(), Some("profile-9"));
    }

    #[test]
    fn empty_registry_gains_the_outdated_profile_and_selects_it() {
        let mut registry = ProfileRegistry::default();

        let changed = ensure_legacy_profile(&mut registry);

        assert!(changed);
        assert_eq!(registry.profiles.len(), 1);
        assert_eq!(registry.profiles[0].id, LEGACY_PROFILE_ID);
        assert_eq!(registry.profiles[0].name, "Outdated Profile");
        assert_eq!(registry.active_profile_id, LEGACY_PROFILE_ID);
    }

    #[test]
    fn migration_keeps_a_valid_active_profile_selected() {
        let mut registry = ProfileRegistry {
            profiles: vec![
                Profile {
                    id: LEGACY_PROFILE_ID.into(),
                    name: "Outdated Profile".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
                Profile {
                    id: "profile-1".into(),
                    name: "Logan".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
            ],
            active_profile_id: "profile-1".into(),
        };

        let changed = ensure_legacy_profile(&mut registry);

        assert!(!changed);
        assert_eq!(registry.active_profile_id, "profile-1");
    }

    #[test]
    fn switching_profiles_opens_that_profiles_newest_chat() {
        let mut program = Program {
            profiles: vec![
                Profile {
                    id: LEGACY_PROFILE_ID.into(),
                    name: "Outdated Profile".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
                Profile {
                    id: "profile-1".into(),
                    name: "Logan".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
            ],
            active_profile_id: LEGACY_PROFILE_ID.to_string(),
            saved_chats: vec![
                test_saved_chat("chat-legacy", LEGACY_PROFILE_ID, "2026-01-03T00:00:00Z"),
                test_saved_chat("chat-b", "profile-1", "2026-01-02T00:00:00Z"),
                test_saved_chat("chat-a", "profile-1", "2026-01-01T00:00:00Z"),
            ],
            ..Program::default()
        };

        drop(program.update(Message::SelectProfile("profile-1".into())));

        assert_eq!(program.active_profile_id, "profile-1");
        assert_eq!(program.current_chat_id, "chat-b");
        assert!(!program.profile_menu_open);
    }

    #[test]
    fn switching_to_an_empty_profile_starts_a_fresh_chat() {
        let mut program = Program {
            profiles: vec![
                Profile {
                    id: LEGACY_PROFILE_ID.into(),
                    name: "Outdated Profile".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
                Profile {
                    id: "profile-1".into(),
                    name: "Logan".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
            ],
            active_profile_id: LEGACY_PROFILE_ID.to_string(),
            saved_chats: vec![test_saved_chat(
                "chat-legacy",
                LEGACY_PROFILE_ID,
                "2026-01-03T00:00:00Z",
            )],
            ..Program::default()
        };

        drop(program.update(Message::SelectProfile("profile-1".into())));

        assert_eq!(program.active_profile_id, "profile-1");
        assert!(program
            .saved_chats
            .iter()
            .all(|chat| chat.id != program.current_chat_id));
        assert!(!program.temporary_chat);
        assert!(program.chat_messages_cache.is_empty());
    }

    #[test]
    fn profiles_with_chats_cannot_be_deleted() {
        let mut program = Program {
            profiles: vec![
                Profile {
                    id: LEGACY_PROFILE_ID.into(),
                    name: "Outdated Profile".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
                Profile {
                    id: "profile-1".into(),
                    name: "Logan".into(),
                    user_name: String::new(),
                    custom_instructions: String::new(),
                },
            ],
            active_profile_id: LEGACY_PROFILE_ID.to_string(),
            saved_chats: vec![test_saved_chat("chat-1", "profile-1", "2026-01-01T00:00:00Z")],
            ..Program::default()
        };

        drop(program.update(Message::DeleteProfile("profile-1".into())));
        assert!(program.profiles.iter().any(|profile| profile.id == "profile-1"));

        // The active profile is protected as well.
        drop(program.update(Message::DeleteProfile(LEGACY_PROFILE_ID.into())));
        assert!(program
            .profiles
            .iter()
            .any(|profile| profile.id == LEGACY_PROFILE_ID));

        program.saved_chats.clear();
        drop(program.update(Message::DeleteProfile("profile-1".into())));
        assert!(program
            .profiles
            .iter()
            .all(|profile| profile.id != "profile-1"));
    }

    #[test]
    fn new_chats_are_saved_into_the_active_profile() {
        let default_program = Program::default();
        let user_information = UserInformation {
            chat_history: Arc::new(Mutex::new(CurrentChat {
                chats: Vec::new(),
                messages: vec![Correspondence::User {
                    text: "Hello there".to_string(),
                    images: Vec::new(),
                }],
                bot_responding: false,
            })),
            ..default_program.user_information.clone()
        };
        let mut program = Program {
            profiles: vec![Profile {
                id: "profile-1".into(),
                name: "Logan".into(),
                user_name: String::new(),
                custom_instructions: String::new(),
            }],
            active_profile_id: "profile-1".to_string(),
            current_chat_id: "chat-new".to_string(),
            open_chat_dirty: true,
            user_information,
            ..default_program
        };

        program.save_open_chat();

        let saved = program
            .saved_chats
            .iter()
            .find(|chat| chat.id == "chat-new")
            .expect("new chat should be saved");
        assert_eq!(saved.profile.as_deref(), Some("profile-1"));
    }

    #[test]
    fn system_prompt_shares_profile_user_name_but_not_display_name() {
        let program = Program {
            profiles: vec![Profile {
                id: "profile-1".into(),
                name: "Secret label".into(),
                user_name: "Logan".into(),
                custom_instructions: "Answer like a pirate.".into(),
            }],
            active_profile_id: "profile-1".to_string(),
            ..Program::default()
        };

        let (user_name, instructions) = program.profile_prompt_extras("profile-1");
        let prompt = program.dynamic_prompt_settings.apply(
            "You are helpful.",
            chrono::Local::now(),
            &user_name,
            &instructions,
        );

        assert!(prompt.contains("The user's name is Logan."));
        assert!(prompt.contains("Answer like a pirate."));
        assert!(!prompt.contains("Secret label"));
    }

    #[test]
    fn confirming_profile_edits_updates_name_user_name_and_instructions() {
        let mut program = Program {
            profiles: vec![Profile {
                id: "profile-1".into(),
                name: "Work".into(),
                user_name: String::new(),
                custom_instructions: String::new(),
            }],
            active_profile_id: "profile-1".to_string(),
            ..Program::default()
        };

        drop(program.update(Message::StartEditProfile("profile-1".into())));
        drop(program.update(Message::ProfileEditNameChanged("Personal".into())));
        drop(program.update(Message::ProfileEditUserNameChanged("Logan".into())));
        drop(program.update(Message::ProfileEditInstructionsChanged(
            "Keep it casual.".into(),
        )));
        drop(program.update(Message::ConfirmProfileEdits));

        let profile = &program.profiles[0];
        assert_eq!(profile.name, "Personal");
        assert_eq!(profile.user_name, "Logan");
        assert_eq!(profile.custom_instructions, "Keep it casual.");
        assert!(program.editing_profile_id.is_none());
    }

    #[test]
    fn system_prompt_phrases_user_name_as_an_instruction() {
        let program = Program {
            profiles: vec![Profile {
                id: "profile-1".into(),
                name: "Label".into(),
                user_name: "Logan".into(),
                custom_instructions: String::new(),
            }],
            active_profile_id: "profile-1".to_string(),
            ..Program::default()
        };

        let (user_name, instructions) = program.profile_prompt_extras("profile-1");
        let prompt = program.dynamic_prompt_settings.apply(
            "You are helpful.",
            chrono::Local::now(),
            &user_name,
            &instructions,
        );

        assert!(prompt.contains(
            "The user's name is Logan. Use this name when addressing the user."
        ));
    }

    #[test]
    fn conversation_context_names_the_user_the_profile_provides() {
        let prompt = conversation_context_prompt(
            "User: Hello there",
            "Logan",
            "What is my name?",
        );

        assert!(prompt
            .contains("a conversation between an AI language model and Logan. You are the AI language model:"));
        assert!(prompt.contains("User: Hello there"));
        assert!(prompt.contains("What is my name?"));
    }

    #[test]
    fn conversation_context_falls_back_to_anonymous_user_without_a_name() {
        let prompt = conversation_context_prompt("User: Hello there", "   ", "Hi");

        assert!(prompt
            .contains("a conversation between an AI language model and a User. You are the AI language model:"));
    }

    #[test]
    fn pinning_moves_a_chat_to_the_pinned_section() {
        let mut program = Program {
            profiles: vec![Profile {
                id: "profile-1".into(),
                name: "Logan".into(),
                user_name: String::new(),
                custom_instructions: String::new(),
            }],
            active_profile_id: "profile-1".to_string(),
            saved_chats: vec![
                test_saved_chat("chat-a", "profile-1", "2026-01-03T00:00:00Z"),
                test_saved_chat("chat-b", "profile-1", "2026-01-02T00:00:00Z"),
                test_saved_chat("chat-c", "profile-1", "2026-01-01T00:00:00Z"),
            ],
            ..Program::default()
        };

        drop(program.update(Message::ToggleChatPin("chat-c".into())));

        let ids: Vec<&str> = program.saved_chats.iter().map(|chat| chat.id.as_str()).collect();
        assert_eq!(ids, vec!["chat-c", "chat-a", "chat-b"]);
        assert!(program.saved_chats[0].pinned);
    }

    #[test]
    fn unpinning_returns_a_chat_to_its_place_in_time() {
        let mut program = Program {
            profiles: vec![Profile {
                id: "profile-1".into(),
                name: "Logan".into(),
                user_name: String::new(),
                custom_instructions: String::new(),
            }],
            active_profile_id: "profile-1".to_string(),
            saved_chats: vec![
                test_saved_chat("chat-b", "profile-1", "2026-01-02T00:00:00Z"),
                test_saved_chat("chat-a", "profile-1", "2026-01-03T00:00:00Z"),
                test_saved_chat("chat-c", "profile-1", "2026-01-01T00:00:00Z"),
            ],
            ..Program::default()
        };
        program.saved_chats[0].pinned = true;

        // chat-b is pinned at the top; unpinning should slot it back by its
        // updated_at (2026-01-02) between chat-a (01-03) and chat-c (01-01).
        drop(program.update(Message::ToggleChatPin("chat-b".into())));

        let ids: Vec<&str> = program.saved_chats.iter().map(|chat| chat.id.as_str()).collect();
        assert_eq!(ids, vec!["chat-a", "chat-b", "chat-c"]);
        assert!(program.saved_chats.iter().all(|chat| !chat.pinned));
    }
}
