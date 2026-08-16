#![windows_subsystem = "windows"]

use std::cmp::Ordering as ComparisonOrdering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::Local;
use iced::{Element, Point, Size, Subscription, Task, Theme, clipboard, keyboard, mouse, time};
use iced_widget::markdown;
use ollama_rs::Ollama;
use rustrict::{Censor, Type};
mod app;
mod gui;
mod inference;
mod tools;

use crate::app::{
    AppState, Channels, ChatImage, Correspondence, CurrentChat, DebugMessage,
    DynamicPromptSettings, FontFamily, LEGACY_PROFILE_ID, LEGACY_PROFILE_NAME, Language, Profile,
    ProfileRegistry, Prompt, SavedChat, SystemPrompt, ThinkingLevel, UserInformation,
};
use crate::inference::{
    BackendConnections, EncodedImage, InferenceBackend, OpenAiStreamLine,
    api_url as backend_api_url, base_url as backend_base_url, decode_openai_stream_line,
    direct_request_body, model_names,
};
use crate::tools::web_search::{
    ToolLoopProgress, ToolLoopRequest, WebSearchProviderKind, WebSearchSettings, WebSearchState,
    create_search_provider, run_tool_loop, send_inference_request_with_retry, validate_public_url,
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
const LARGE_CODE_UI_BYTES: usize = 4 * 1024;
/// Re-parsing a growing Markdown document becomes expensive once it includes
/// a large code block. Keep the UI responsive by publishing those snapshots
/// less frequently; the final complete snapshot is still always sent.
const LARGE_LIVE_RENDER_BYTES: usize = LARGE_CODE_UI_BYTES;
const LARGE_LIVE_RENDER_MS: u128 = 500;
const MAX_MARKDOWN_IMAGE_BYTES: usize = 12 * 1024 * 1024;
const MAX_MARKDOWN_IMAGE_PIXELS: u64 = 32_000_000;
const SETTINGS_SAVE_DEBOUNCE_MS: u64 = 450;
const SETTINGS_FEEDBACK_DURATION_MS: u64 = 420;
const DEFAULT_MAX_RESPONSE_TOKENS: u32 = 32_768;
const DEFAULT_CONTEXT_TOKENS: u32 = 131_072;
const MIN_RESPONSE_TOKENS: u32 = 512;
const MAX_RESPONSE_TOKENS: u32 = 1_048_576;
const MIN_CONTEXT_TOKENS: u32 = 4_096;
const MAX_CONTEXT_TOKENS: u32 = 4_194_304;
const DEFAULT_TEXT_SIZE: f32 = 15.0;
const MIN_TEXT_SIZE: f32 = 1.0;
const MAX_TEXT_SIZE: f32 = 40.0;
const DEFAULT_SIDEBAR_WIDTH: f32 = 248.0;
const MIN_SIDEBAR_WIDTH: f32 = 196.0;
const MAX_SIDEBAR_WIDTH: f32 = 420.0;
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
}

/// Identifies the Markdown collection that owns a code block. Passing this
/// small handle through a button avoids cloning a potentially multi-megabyte
/// snippet whenever Iced rebuilds the view.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CodeCopyScope {
    ChatMessage(usize),
    ActiveResponse,
    VisionResponse,
}

#[derive(Debug, Clone)]
enum Message {
    ChangeBatchTokens(i32),
    ToggleFastStreaming,
    ToggleChatMenu,
    ToggleConfigDrawer,
    ToggleChatRowMenu(String),
    ToggleWebSearch,
    ToggleTools,
    ToggleMultipleWebSearches,
    ToggleWebSearchTool,
    ToggleFetchWebpageTool,
    ToggleConversationSearchTool,
    ToggleCodeCheckingTool,
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
    CopyResponse(usize),
    CopyCode(CodeCopyScope, usize),
    ToggleThinking(usize),
    ToggleSources(usize),
    UpdateTextSize(f32),
    FontFamilyChange(FontFamily),
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
    CheckCode(CodeCopyScope, usize, String),
    CodeChecked(Result<String, String>),
    DismissCodeCheckResult,
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
    ToggleShowTokensPerSecond,
    ToggleInfoPopupSetting,
    WipeChatHistory,
    ToggleAdvancedSettings,
    BackendChange(InferenceBackend),
    ChangeProtocol(String),
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
    /// Generation speed captured from final stream statistics. OpenVINO's
    /// missing duration is measured locally. The async response loop writes
    /// it; the finish handler reads it when stamping the reply.
    tokens_per_second: Arc<Mutex<Option<f32>>>,
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

#[derive(Clone, Debug)]
struct GenerationChunk {
    response: String,
    done: bool,
    thinking: Option<String>,
    eval_count: Option<u64>,
    eval_duration: Option<u64>,
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
    /// Manual code-check feedback is shown as a dismissible overlay so large
    /// compiler diagnostics never shrink the chat composer.
    code_check_result: Option<DebugMessage>,

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
    show_info_popup: bool,
    prompt: Prompt,
    batch_tokens: i32,
    fast_streaming: bool,
    /// Persisted setting that shows the generation speed under each reply.
    show_tokens_per_second: bool,
    chat_menu_open: bool,
    config_drawer_open: bool,
    chat_row_menu: Option<String>,
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
    tool_settings: crate::tools::ToolSettings,
    web_search_for_chat: bool,
    /// Web-search choice applied to newly started chats. New chats no longer
    /// inherit the persisted global setting; they start OFF and then follow
    /// whatever the user last picked for a chat this session. Deliberately
    /// in-memory only: it is never read from or written to storage.
    new_chat_web_search: bool,
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
    if !registry
        .profiles
        .iter()
        .any(|profile| profile.id == LEGACY_PROFILE_ID)
    {
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
    let partner = if user_name.is_empty() {
        "a User"
    } else {
        user_name
    };
    format!(
        "The following is a conversation between an AI language model and {partner}. You are the AI language model:
    {context}
    [END CONVERSATION CONTEXT]
    Now, the user is sending another message: {prompt}
    Respond:
    "
    )
}

/// Tokens per second from the final stream statistics. Ollama supplies both
/// values directly; OpenVINO supplies the token count and Locoryn measures the
/// generation interval. Neither path is affected by visual token batching.
fn tokens_per_second(eval_count: Option<u64>, eval_duration: Option<u64>) -> Option<f32> {
    let tokens = eval_count? as f64;
    let duration_ns = eval_duration? as f64;
    if tokens <= 0.0 || duration_ns <= 0.0 {
        return None;
    }
    Some((tokens / (duration_ns / 1_000_000_000.0)) as f32)
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
    })
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
/// of scanning the operating system. Load the platform chat, emoji, and symbol
/// fonts explicitly so every appearance choice works and model output has a
/// real fallback instead of rendering unsupported glyphs as empty boxes.
fn fallback_font_bytes() -> Vec<Vec<u8>> {
    let candidates = [
        resource_path("assets/NotoColorEmoji.ttf"),
        resource_path("assets/NotoSansSymbols2-Regular.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\times.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\consola.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\seguiemj.ttf"),
        PathBuf::from(r"C:\Windows\Fonts\seguisym.ttf"),
        PathBuf::from("/System/Library/Fonts/Times.ttc"),
        PathBuf::from("/System/Library/Fonts/Menlo.ttc"),
        PathBuf::from("/System/Library/Fonts/Apple Color Emoji.ttc"),
        PathBuf::from("/System/Library/Fonts/Apple Symbols.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"),
        PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
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
    crate::tools::code_checking::canonical_language(language)
}

fn check_code(language: String, code: String) -> Result<String, String> {
    crate::tools::code_checking::check_code(&language, &code)
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
    let items = markdown::parse(&normalized).collect::<Vec<_>>();
    // Populate character counts while parsing runs off the UI thread for live
    // responses. The preview can then report an exact omission count without
    // rescanning a large snippet during every view rebuild.
    for item in &items {
        if let markdown::Item::CodeBlock { code, .. } = item
            && code.len() > LARGE_CODE_UI_BYTES
        {
            let _ = cached_character_count(code);
            let mut preview_end = code.len().min(8 * 1024);
            while !code.is_char_boundary(preview_end) {
                preview_end -= 1;
            }
            let _ = cached_character_count(&code[..preview_end]);
        }
    }
    items
}

/// Parse a streamed Markdown snapshot without feeding the contents of fenced
/// code blocks through Iced's syntax highlighter. `markdown::parse` highlights
/// while it parses (not while the code-block widget is built), so merely using
/// a monospace widget for live code does not avoid repeatedly highlighting the
/// complete, growing snippet.
///
/// Each fenced block is replaced by a private image placeholder for the live
/// parse, then restored as a code-block item with empty highlighted lines. This
/// prevents even syntax-highlighter initialization on each snapshot while
/// retaining the block's position, language, and exact source. Completed
/// messages still go through `parse_markdown_items` and receive normal syntax
/// highlighting once.
fn parse_live_markdown_items(input: &str) -> Vec<markdown::Item> {
    let normalized = normalize_code_fence_languages(input);
    let (masked, code_blocks) = mask_live_code_blocks(&normalized);
    let mut items = markdown::parse(&masked).collect::<Vec<_>>();
    let mut code_blocks = code_blocks.into_iter().map(Some).collect::<Vec<_>>();
    restore_live_code_blocks(&mut items, &mut code_blocks);
    items
}

const LIVE_CODE_PLACEHOLDER: &str = "locoryn-live-code:";

struct LiveCodeBlock {
    language: Option<String>,
    code: String,
}

fn mask_live_code_blocks(input: &str) -> (String, Vec<LiveCodeBlock>) {
    struct OpenFence {
        marker: u8,
        length: usize,
        indentation: usize,
        block: LiveCodeBlock,
    }

    fn fence(line: &str) -> Option<(u8, usize, usize)> {
        let body = line.trim_end_matches(['\r', '\n']);
        let trimmed = body.trim_start();
        let indentation = body.len() - trimmed.len();
        // CommonMark only recognizes fences indented by at most three spaces.
        if indentation > 3 {
            return None;
        }
        let marker = trimmed.as_bytes().first().copied()?;
        if !matches!(marker, b'`' | b'~') {
            return None;
        }
        let length = trimmed
            .as_bytes()
            .iter()
            .take_while(|character| **character == marker)
            .count();
        (length >= 3).then_some((marker, length, indentation))
    }

    let mut masked = String::with_capacity(input.len().min(16 * 1024));
    let mut code_blocks = Vec::new();
    let mut open: Option<OpenFence> = None;

    for segment in input.split_inclusive('\n') {
        if let Some(active) = &mut open {
            let closing = fence(segment).is_some_and(|(marker, length, _)| {
                marker == active.marker
                    && length >= active.length
                    && segment.trim_end_matches(['\r', '\n']).trim_start()[length..]
                        .trim()
                        .is_empty()
            });
            if closing {
                let body_length = segment.trim_end_matches(['\r', '\n']).len();
                masked.push_str(&segment[body_length..]);
                code_blocks.push(open.take().unwrap().block);
            } else {
                let body_length = segment.trim_end_matches(['\r', '\n']).len();
                let (body, line_ending) = segment.split_at(body_length);
                let removable_indent = body
                    .as_bytes()
                    .iter()
                    .take(active.indentation)
                    .take_while(|character| **character == b' ')
                    .count();
                active.block.code.push_str(&body[removable_indent..]);
                active.block.code.push_str(line_ending);
            }
            continue;
        }

        let Some((marker, length, indentation)) = fence(segment) else {
            masked.push_str(segment);
            continue;
        };
        let body = segment.trim_end_matches(['\r', '\n']);
        let info = &body[indentation + length..];
        // Backticks are forbidden in the info string of a backtick fence.
        if marker == b'`' && info.contains('`') {
            masked.push_str(segment);
            continue;
        }

        for _ in 0..indentation {
            masked.push(' ');
        }
        masked.push_str("![locoryn live code](");
        masked.push_str(LIVE_CODE_PLACEHOLDER);
        masked.push_str(&code_blocks.len().to_string());
        masked.push(')');
        let body_length = segment.trim_end_matches(['\r', '\n']).len();
        masked.push_str(&segment[body_length..]);
        open = Some(OpenFence {
            marker,
            length,
            indentation,
            block: LiveCodeBlock {
                language: (!info.trim().is_empty()).then(|| info.trim().to_string()),
                code: String::new(),
            },
        });
    }

    if let Some(active) = open {
        code_blocks.push(active.block);
    }
    (masked, code_blocks)
}

fn restore_live_code_blocks(
    items: &mut [markdown::Item],
    code_blocks: &mut [Option<LiveCodeBlock>],
) {
    for item in items {
        let placeholder_index = match item {
            markdown::Item::Image { url, .. } => url
                .strip_prefix(LIVE_CODE_PLACEHOLDER)
                .and_then(|index| index.parse::<usize>().ok()),
            _ => None,
        };
        if let Some(index) = placeholder_index
            && let Some(block) = code_blocks.get_mut(index).and_then(Option::take)
        {
            *item = markdown::Item::CodeBlock {
                language: block.language,
                code: block.code,
                lines: Vec::new(),
            };
            continue;
        }

        match item {
            markdown::Item::Quote(items) => restore_live_code_blocks(items, code_blocks),
            markdown::Item::List { bullets, .. } => {
                for bullet in bullets {
                    let items = match bullet {
                        markdown::Bullet::Point { items }
                        | markdown::Bullet::Task { items, .. } => items,
                    };
                    restore_live_code_blocks(items, code_blocks);
                }
            }
            _ => {}
        }
    }
}

fn character_count_cache() -> &'static Mutex<HashMap<(usize, usize), usize>> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, usize), usize>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Count Unicode scalar values once per parsed code buffer. The pointer and
/// length identify the immutable Markdown-owned string while it is displayed.
pub(crate) fn cached_character_count(text: &str) -> usize {
    let key = (text.as_ptr() as usize, text.len());
    if let Ok(cache) = character_count_cache().lock()
        && let Some(count) = cache.get(&key)
    {
        return *count;
    }

    let count = text.chars().count();
    if let Ok(mut cache) = character_count_cache().lock() {
        // This is only presentation metadata. Bound it so old streamed
        // snapshots cannot retain a process-long cache of pointer entries.
        if cache.len() >= 256 {
            cache.clear();
        }
        cache.insert(key, count);
    }
    count
}

fn decode_generation_line(
    input: &str,
) -> Result<(GenerationChunk, Option<String>), serde_json::Error> {
    let value = serde_json::from_str::<serde_json::Value>(input)?;
    let done_reason = value
        .get("done_reason")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    Ok((
        GenerationChunk {
            response: value
                .get("response")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
            done: value
                .get("done")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            thinking: value
                .get("thinking")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            eval_count: value.get("eval_count").and_then(serde_json::Value::as_u64),
            eval_duration: value
                .get("eval_duration")
                .and_then(serde_json::Value::as_u64),
        },
        done_reason,
    ))
}

enum InferenceStreamLine {
    Chunk(GenerationChunk, Option<String>),
    Done,
    Ignore,
}

fn decode_inference_stream_line(
    backend: InferenceBackend,
    input: &str,
) -> Result<InferenceStreamLine, String> {
    match backend {
        InferenceBackend::Ollama => decode_generation_line(input)
            .map(|(chunk, reason)| InferenceStreamLine::Chunk(chunk, reason))
            .map_err(|error| error.to_string()),
        InferenceBackend::OpenVino => match decode_openai_stream_line(input)? {
            OpenAiStreamLine::Done => Ok(InferenceStreamLine::Done),
            OpenAiStreamLine::Ignore => Ok(InferenceStreamLine::Ignore),
            OpenAiStreamLine::Event(event) => Ok(InferenceStreamLine::Chunk(
                GenerationChunk {
                    response: event.content,
                    done: event.finish_reason.is_some(),
                    thinking: (!event.reasoning.is_empty()).then_some(event.reasoning),
                    eval_count: event.completion_tokens,
                    eval_duration: None,
                },
                event.finish_reason,
            )),
        },
    }
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
            tokens_per_second: None,
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

    fn persist_tool_settings(&mut self) {
        match serde_json::to_value(&self.tool_settings) {
            Ok(value) => self.persist_setting_value("tools", value),
            Err(error) => self.set_debug_message(DebugMessage {
                message: format!("Could not save tool settings: {error}"),
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

    fn persist_backend_connections(&mut self) {
        match serde_json::to_value(&self.user_information.backend_connections) {
            Ok(value) => self.persist_setting_value("backend_connections", value),
            Err(error) => self.set_debug_message(DebugMessage {
                message: format!("Could not save inference connections: {error}"),
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
        // New chats start with the session's remembered choice, which begins
        // OFF and is never persisted.
        self.web_search_for_chat = self.new_chat_web_search;
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
                .unwrap_or(self.new_chat_web_search);
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
                job.parsed_markdown = parse_live_markdown_items(&progress.answer);
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

    fn code_block_text(items: &[markdown::Item], code_block_index: usize) -> Option<String> {
        items
            .iter()
            .filter_map(|item| match item {
                markdown::Item::CodeBlock { code, .. } => Some(code.clone()),
                _ => None,
            })
            .nth(code_block_index)
    }

    fn copy_text(&mut self, input: Option<String>) -> Task<Message> {
        let Some(input) = input.filter(|input| !input.trim().is_empty()) else {
            self.set_debug_message(DebugMessage {
                message: "Nothing to copy yet.".to_string(),
                is_error: true,
            });
            return Task::none();
        };

        self.last_copied_text = Some(input.clone());
        self.last_copied_at = Some(Instant::now());
        self.set_debug_message(DebugMessage {
            message: "Copied to clipboard.".to_string(),
            is_error: false,
        });
        clipboard::write::<Message>(input)
    }

    fn code_block_for_scope(
        &self,
        scope: CodeCopyScope,
        code_block_index: usize,
    ) -> Option<String> {
        match scope {
            CodeCopyScope::ChatMessage(message_index) => self
                .chat_markdown_cache
                .get(message_index)
                .and_then(|items| Self::code_block_text(items, code_block_index)),
            CodeCopyScope::ActiveResponse => self
                .active_prompts
                .get(&self.current_chat_id)
                .and_then(|job| Self::code_block_text(&job.parsed_markdown, code_block_index)),
            CodeCopyScope::VisionResponse => self
                .vision_responses
                .get(&self.current_chat_id)
                .and_then(|response| Self::code_block_text(&response.markdown, code_block_index)),
        }
    }

    fn copy_code_block(&mut self, scope: CodeCopyScope, code_block_index: usize) -> Task<Message> {
        self.copy_text(self.code_block_for_scope(scope, code_block_index))
    }

    fn apply_response_metadata(
        chat: &mut CurrentChat,
        response_start_index: usize,
        model_name: &str,
        elapsed_seconds: u64,
        tokens_per_second: Option<f32>,
    ) {
        if let Some(Correspondence::Bot {
            model: stored_model,
            thinking_seconds,
            tokens_per_second: stored_tokens_per_second,
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
            if stored_tokens_per_second.is_none() {
                *stored_tokens_per_second = tokens_per_second;
            }
        }
    }

    fn finalize_response_metadata(job: &ActivePrompt) {
        let elapsed_seconds = job.started_at.elapsed().as_secs().max(1);
        let tokens_per_second = job.tokens_per_second.lock().ok().and_then(|stats| *stats);
        if let Ok(mut chat) = job.chat_history.lock() {
            Self::apply_response_metadata(
                &mut chat,
                job.response_start_index,
                &job.model_name,
                elapsed_seconds,
                tokens_per_second,
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

        // Backpressure bounds token memory if preparing a large response is
        // temporarily slower than the inference stream.
        let (tx, mut rx) = tokio::sync::mpsc::channel::<GenerationChunk>(64);
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
                    markdown: parse_live_markdown_items(&visible),
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

                let elapsed_ms = last_render_time.elapsed().as_millis();
                if buffer.len() >= LARGE_LIVE_RENDER_BYTES && elapsed_ms < LARGE_LIVE_RENDER_MS {
                    continue;
                }
                if fast_streaming {
                    // "Fast" remains visually immediate without reparsing the
                    // entire growing answer more than roughly once per frame.
                    if elapsed_ms < LIVE_RENDER_MS {
                        continue;
                    }
                } else if total_tokens < batch_tokens && elapsed_ms < 250 {
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

        let backend = self.user_information.backend;
        let backend_location = self.user_information.active_connection().clone();
        let backend_chat_url =
            match backend_api_url(backend, &backend_location, backend.chat_path()) {
                Ok(url) => url,
                Err(message) => {
                    self.set_debug_message(DebugMessage {
                        message,
                        is_error: true,
                    });
                    return Task::none();
                }
            };
        let backend_generate_url = match backend_api_url(
            backend,
            &backend_location,
            match backend {
                InferenceBackend::Ollama => "/api/generate",
                InferenceBackend::OpenVino => backend.chat_path(),
            },
        ) {
            Ok(url) => url,
            Err(message) => {
                self.set_debug_message(DebugMessage {
                    message,
                    is_error: true,
                });
                return Task::none();
            }
        };

        // Clone the attachment into the request/chat first. The composer owns its
        // copy until the submission has been accepted, avoiding a transient blank
        // preview while the async request is being prepared.
        let attached_images = self.pending_images.clone();
        let encoded_images = attached_images
            .iter()
            .map(|image| EncodedImage {
                mime_type: image.mime_type.clone(),
                data: BASE64.encode(&image.bytes),
            })
            .collect::<Vec<_>>();
        let had_image = !attached_images.is_empty();
        let filtering = self.app_state.filtering;
        let code_checking_enabled = self.code_checking_enabled;
        let user_info = self.user_information.clone();
        let web_search_enabled = self.web_search_for_chat;
        let mut web_search_settings = self.web_search_settings.clone();
        web_search_settings.enabled = web_search_enabled;
        // The per-chat Web switch scopes only network tools.  It must not
        // disable the local Past Chats tool, nor override the global Enable
        // Tools preference from Settings.
        let mut tool_settings = self
            .tool_settings
            .clone()
            .for_chat_web_enabled(web_search_enabled);
        // The per-tool setting records the user's preference, but the native
        // tool is only exposed for this request after the separate advanced
        // consent switch has also been enabled.
        if !code_checking_enabled {
            tool_settings.code_checking = false;
        }
        let (web_search_state_sender, web_search_state_receiver) = crossbeam_channel::unbounded();
        let (web_progress_sender, web_progress_receiver) =
            tokio::sync::watch::channel(ToolLoopProgress::default());
        // Generation speed is captured independently of render batching. For
        // OpenVINO, the stream timer fills in the duration paired with its
        // completion-token count.
        let tokens_per_second_stats = Arc::new(Mutex::new(None::<f32>));
        let chat_id = self.current_chat_id.clone();
        let completion_chat_id = chat_id.clone();
        let notice_chat_id = chat_id.clone();
        let chat_notice_sender = self.chat_notice_sender.clone();
        let chat_storage_dir = self.chat_storage_dir.clone();
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
                tokens_per_second: Arc::clone(&tokens_per_second_stats),
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
                let to_send_prompt: String = if user_info.current_chat_history_enabled {
                    conversation_context_prompt(
                        &user_info.chat_history.lock().unwrap().unravel(),
                        &prompt_user_name,
                        &prompt,
                    )
                } else {
                    prompt.clone()
                };

                if tool_settings.any_tool_enabled() {
                    let provider = if tool_settings.web_tools_enabled() {
                        match create_search_provider(&web_search_settings) {
                            Ok(provider) => Some(provider),
                            Err(error) => {
                                let api_key = web_search_settings.resolved_api_key();
                                let message = error.detailed_user_message(api_key.as_deref());
                                send_chat_notice(
                                    &chat_notice_sender,
                                    &notice_chat_id,
                                    DebugMessage {
                                        message: format!(
                                            "Web search is unavailable for this response: {message}. The model can still use other enabled tools."
                                        ),
                                        is_error: true,
                                    },
                                );
                                // Do not make an optional provider setup error abort a
                                // local tool call (notably Past Chats) or the reply.
                                // Removing these definitions also prevents the model
                                // from repeatedly requesting a tool that cannot run.
                                tool_settings.web_search = false;
                                tool_settings.fetch_webpage = false;
                                None
                            }
                        }
                    } else {
                        None
                    };
                    let result = run_tool_loop(ToolLoopRequest {
                        backend,
                        chat_url: backend_chat_url,
                        model: user_info.model.clone().unwrap(),
                        prompt: to_send_prompt,
                        system_prompt: system_prompt.clone(),
                        temperature: user_info.temperature / 10.0,
                        context_tokens: user_info.context_tokens,
                        max_response_tokens: user_info.max_response_tokens,
                        images: encoded_images.clone(),
                        thinking: user_info.thinking_level.api_value(),
                        settings: web_search_settings.clone(),
                        tool_settings: tool_settings.clone(),
                        code_checking_enabled,
                        provider,
                        state_sender: web_search_state_sender.clone(),
                        progress_sender: web_progress_sender,
                        cancel: Arc::clone(&cancel),
                        chat_storage_dir: Some(chat_storage_dir),
                    })
                    .await;

                    match result {
                        Ok(result) => {
                            // Sources are the durable evidence that this response actually
                            // used a web tool. The tool loop can also run local tools (such as
                            // Past Chats or code checking), which must not earn a WEB badge.
                            let web_search_used = !result.sources.is_empty();
                            *tokens_per_second_stats.lock().unwrap() =
                                tokens_per_second(result.eval_count, result.eval_duration);
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
                                .send(GenerationChunk {
                                    response: complete_response.clone(),
                                    done: true,
                                    eval_count: None,
                                    eval_duration: None,
                                    thinking: None,
                                })
                                .await;
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
                                    tokens_per_second: None,
                                    sources: result.sources,
                                    web_search_used,
                                },
                            );
                        }
                        Err(crate::tools::web_search::WebSearchError::Cancelled) => {}
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
                                    tokens_per_second: None,
                                    sources: Vec::new(),
                                    web_search_used: false,
                                },
                            );
                        }
                    }
                    user_info.chat_history.lock().unwrap().bot_responding = false;
                    return;
                }

                println!("System prompt: {}", system_prompt.clone());
                let request_body = direct_request_body(
                    backend,
                    user_info.model.as_deref().unwrap_or_default(),
                    to_send_prompt,
                    system_prompt,
                    &encoded_images,
                    &user_info.thinking_level.api_value(),
                    user_info.temperature / 10.0,
                    user_info.context_tokens,
                    user_info.max_response_tokens,
                );

                let client = reqwest::Client::new();
                let response = send_inference_request_with_retry(
                    &client,
                    &backend_generate_url,
                    &request_body,
                    &cancel,
                )
                .await;
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
                        let message = format!(
                            "{} rejected the request ({status}): {detail}",
                            backend.server_name()
                        );
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
                        let message = format!("Could not reach {}: {error}", backend.server_name());
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
                // OpenVINO's OpenAI-compatible stream reports completion-token
                // usage but not an evaluation duration. Time from the first
                // generated content through the usage terminator so the same
                // footer can still be populated for that backend.
                let mut openvino_generation_started_at = None::<Instant>;
                let mut openvino_completion_tokens = None::<u64>;

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
                        match decode_inference_stream_line(backend, &line) {
                            Ok(InferenceStreamLine::Chunk(mut token, done_reason)) => {
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

                                // Both backends can return reasoning separately, while
                                // some models emit literal <think> tags. Normalize both.
                                if let Some(thinking) = token.thinking.take()
                                    && !thinking.is_empty()
                                {
                                    token.response =
                                        format!("<think>{thinking}</think>{}", token.response);
                                }

                                if backend == InferenceBackend::OpenVino {
                                    if openvino_generation_started_at.is_none()
                                        && !token.response.is_empty()
                                    {
                                        openvino_generation_started_at = Some(Instant::now());
                                    }
                                    if let Some(completion_tokens) = token.eval_count {
                                        openvino_completion_tokens = Some(completion_tokens);
                                    }
                                }

                                final_response.push(token.response.clone());

                                // Capture backend timing statistics when supplied.
                                if token.done {
                                    *tokens_per_second_stats.lock().unwrap() =
                                        tokens_per_second(token.eval_count, token.eval_duration);
                                }

                                // Filtering must see the complete response because a
                                // backend can split a word across arbitrary chunks.
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
                            Ok(InferenceStreamLine::Done) => break 'response_stream,
                            Ok(InferenceStreamLine::Ignore) => {}
                            Err(e) => {
                                eprintln!("Error decoding {} response: {e}", backend.server_name());
                                send_chat_notice(
                                    &chat_notice_sender,
                                    &notice_chat_id,
                                    DebugMessage {
                                        message: format!(
                                            "{} returned an invalid streaming response",
                                            backend.server_name()
                                        ),
                                        is_error: true,
                                    },
                                );
                            }
                        }
                    }
                }

                let was_cancelled = cancel.load(Ordering::Relaxed);
                // Accept a final unterminated event from proxies or older servers.
                let trailing_line = stream_buffer.trim();
                if !was_cancelled && !trailing_line.is_empty() {
                    match decode_inference_stream_line(backend, trailing_line) {
                        Ok(InferenceStreamLine::Chunk(mut token, done_reason)) => {
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
                            if backend == InferenceBackend::OpenVino {
                                if openvino_generation_started_at.is_none()
                                    && !token.response.is_empty()
                                {
                                    openvino_generation_started_at = Some(Instant::now());
                                }
                                if let Some(completion_tokens) = token.eval_count {
                                    openvino_completion_tokens = Some(completion_tokens);
                                }
                            }
                            final_response.push(token.response.clone());
                            if token.done {
                                *tokens_per_second_stats.lock().unwrap() =
                                    tokens_per_second(token.eval_count, token.eval_duration);
                            }
                            if !filtering {
                                let _ = tx.send(token).await;
                            }
                        }
                        Ok(InferenceStreamLine::Done | InferenceStreamLine::Ignore) => {}
                        Err(error) => {
                            eprintln!(
                                "Error decoding final {} response: {error}",
                                backend.server_name()
                            );
                            send_chat_notice(
                                &chat_notice_sender,
                                &notice_chat_id,
                                DebugMessage {
                                    message: format!(
                                        "{} returned an invalid final streaming response",
                                        backend.server_name()
                                    ),
                                    is_error: true,
                                },
                            );
                        }
                    }
                }

                if backend == InferenceBackend::OpenVino
                    && let Some(started_at) = openvino_generation_started_at
                {
                    let duration_ns = u64::try_from(started_at.elapsed().as_nanos()).ok();
                    *tokens_per_second_stats.lock().unwrap() =
                        tokens_per_second(openvino_completion_tokens, duration_ns);
                }

                if !was_cancelled && final_response.concat().trim().is_empty() {
                    let message = format!(
                        "{} ended the response without returning content.",
                        backend.server_name()
                    );
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
                        .send(GenerationChunk {
                            response: filtered,
                            done: true,
                            eval_count: None,
                            eval_duration: None,
                            thinking: None,
                        })
                        .await;
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
                            model: user_info.model.clone(),
                            thinking_seconds: None,
                            tokens_per_second: None,
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

            Message::ToggleConfigDrawer => {
                self.config_drawer_open = !self.config_drawer_open;
                Task::none()
            }

            Message::ToggleChatRowMenu(id) => {
                self.chat_row_menu = if self.chat_row_menu.as_deref() == Some(id.as_str()) {
                    None
                } else {
                    Some(id)
                };
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
                    // The user's latest explicit choice becomes the default
                    // for chats started later in this session (in-memory only).
                    self.new_chat_web_search = self.web_search_for_chat;
                    self.persist_current_chat_web_search_setting();
                }
                self.persist_web_search_settings();
                Task::none()
            }

            Message::ToggleTools => {
                self.tool_settings.enabled = !self.tool_settings.enabled;
                self.persist_tool_settings();
                Task::none()
            }

            Message::ToggleMultipleWebSearches => {
                self.web_search_settings.allow_multiple_searches =
                    !self.web_search_settings.allow_multiple_searches;
                self.persist_web_search_settings();
                Task::none()
            }

            Message::ToggleWebSearchTool => {
                self.tool_settings.web_search = !self.tool_settings.web_search;
                self.persist_tool_settings();
                Task::none()
            }

            Message::ToggleFetchWebpageTool => {
                self.tool_settings.fetch_webpage = !self.tool_settings.fetch_webpage;
                self.persist_tool_settings();
                Task::none()
            }

            Message::ToggleConversationSearchTool => {
                self.tool_settings.conversation_search = !self.tool_settings.conversation_search;
                self.persist_tool_settings();
                Task::none()
            }

            Message::ToggleCodeCheckingTool => {
                self.tool_settings.code_checking = !self.tool_settings.code_checking;
                self.persist_tool_settings();
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
                // Remember the user's latest choice as the default for chats
                // started later this session. In-memory only: never persisted.
                self.new_chat_web_search = self.web_search_for_chat;
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
                    (value.round() as usize).clamp(1, crate::tools::web_search::MAX_RESULT_LIMIT);
                self.persist_web_search_settings();
                self.trigger_settings_feedback(SettingsFeedbackTarget::SearchResultLimit);
                Task::none()
            }

            Message::WebSearchMaximumSearchesChange(value) => {
                self.web_search_settings.maximum_searches = (value.round() as usize)
                    .clamp(1, crate::tools::web_search::MAX_CONFIGURABLE_SEARCHES);
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
                    (value.round() as usize).min(crate::tools::web_search::MAX_CONFIGURABLE_PAGES);
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
                self.web_search_settings.tool_iteration_limit = (value.round() as usize).clamp(
                    2,
                    crate::tools::web_search::MAX_CONFIGURABLE_TOOL_ITERATIONS,
                );
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
                    .take(crate::tools::web_search::MAX_CUSTOM_RESEARCH_INSTRUCTIONS_CHARS)
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
                self.chat_row_menu = None;
                self.current_chat_id = Self::new_chat_id();
                self.temporary_chat = false;
                self.web_search_for_chat = self.new_chat_web_search;
                self.clear_open_chat();
                self.begin_page_transition();
                Task::none()
            }

            Message::OpenChat(id) => {
                self.chat_row_menu = None;
                self.open_chat(id)
            }

            Message::DeleteChat(id) => {
                self.chat_row_menu = None;
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
                    self.web_search_for_chat = self.new_chat_web_search;
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
                    self.web_search_for_chat = self.new_chat_web_search;
                    self.clear_open_chat();
                    self.begin_page_transition();
                }
                Task::none()
            }

            Message::ToggleChatPin(id) => {
                self.chat_row_menu = None;
                if let Some(index) = self.saved_chats.iter().position(|chat| chat.id == id) {
                    let mut chat = self.saved_chats.remove(index);
                    chat.pinned = !chat.pinned;
                    let pinned_count = self.saved_chats.iter().filter(|chat| chat.pinned).count();
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
                    self.web_search_for_chat = self.new_chat_web_search;
                    self.clear_open_chat();
                } else {
                    self.save_open_chat();
                    self.temporary_chat = true;
                    self.current_chat_id = Self::new_chat_id();
                    self.web_search_for_chat = self.new_chat_web_search;
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
                    let backend_state = Arc::clone(&self.app_state.backend_state);
                    let user_info = self.user_information.clone();

                    return Task::perform(
                        async move {
                            let backend = user_info.backend;
                            println!("Checking {} status...", backend.server_name());
                            let url = match backend_api_url(
                                backend,
                                user_info.active_connection(),
                                backend.status_path(),
                            ) {
                                Ok(url) => url,
                                Err(error) => {
                                    println!("Invalid {} address: {error}", backend.server_name());
                                    *backend_state.lock().unwrap() = "Offline".to_string();
                                    return;
                                }
                            };

                            match reqwest::get(url).await {
                                Ok(response) => {
                                    println!("API responded with status: {}", response.status());

                                    if response.status().is_success() {
                                        *backend_state.lock().unwrap() = match backend {
                                            InferenceBackend::Ollama => response
                                                .json::<serde_json::Value>()
                                                .await
                                                .ok()
                                                .and_then(|json| {
                                                    json.get("version")
                                                        .and_then(serde_json::Value::as_str)
                                                        .map(|version| {
                                                            format!("Online (v{version})")
                                                        })
                                                })
                                                .unwrap_or_else(|| {
                                                    "Online (unknown version)".to_string()
                                                }),
                                            InferenceBackend::OpenVino => {
                                                "Online (OpenVINO)".to_string()
                                            }
                                        };
                                    } else {
                                        *backend_state.lock().unwrap() = "Offline".to_string();
                                    }
                                }
                                Err(err) => {
                                    println!("Failed to reach API: {}", err);
                                    *backend_state.lock().unwrap() = "Offline".to_string();
                                }
                            }
                        },
                        Message::AsyncResult,
                    );
                } else if self.current_tick == BOT_LIST_TICK {
                    let backend = self.user_information.backend;
                    let location = self.user_information.active_connection().clone();
                    let url = match backend_api_url(backend, &location, backend.models_path()) {
                        Ok(url) => url,
                        Err(message) => {
                            self.set_debug_message(DebugMessage {
                                message,
                                is_error: true,
                            });
                            return Task::none();
                        }
                    };
                    let bots_list = Arc::clone(&self.app_state.bots_list);
                    let channels = self.channels.clone();

                    return Task::perform(
                        async move {
                            let result = match reqwest::get(url).await {
                                Ok(response) if response.status().is_success() => response
                                    .json::<serde_json::Value>()
                                    .await
                                    .map_err(|error| error.to_string())
                                    .and_then(|json| model_names(backend, &json)),
                                Ok(response) => Err(format!("HTTP {}", response.status())),
                                Err(error) => Err(error.to_string()),
                            };
                            match result {
                                Ok(names) => {
                                    *bots_list.lock().unwrap() = names;
                                }
                                Err(e) => {
                                    Channels::send_request_to_channel(
                                        Arc::clone(&channels.debug_channel),
                                        DebugMessage {
                                            message: format!(
                                                "Could not list models from {}",
                                                backend.server_name()
                                            ),
                                            is_error: true,
                                        },
                                    );
                                    bots_list.lock().unwrap().clear();
                                    println!("Error listing models: {e}");
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

                if content_changed {
                    self.queue_missing_markdown_images()
                } else {
                    Task::none()
                }
            }

            Message::ChangeIp(ip) => {
                self.user_information.active_connection_mut().ip = ip;
                self.persist_backend_connections();
                Task::none()
            }

            Message::ChangeProtocol(protocol) => {
                self.user_information.active_connection_mut().protocol =
                    protocol.trim().trim_end_matches("://").to_ascii_lowercase();
                self.persist_backend_connections();
                Task::none()
            }

            Message::ChangePort(port) => {
                self.user_information.active_connection_mut().port = port;
                self.persist_backend_connections();
                Task::none()
            }

            Message::BackendChange(backend) => {
                if backend == self.user_information.backend {
                    return Task::none();
                }
                self.user_information.backend = backend;
                self.user_information.model = None;
                self.user_information.thinking_level = ThinkingLevel::Off;
                self.user_information.thinking_levels = vec![ThinkingLevel::Off];
                self.user_information.thinking_supported = None;
                self.user_information.vision_supported = None;
                self.app_state.bots_list.lock().unwrap().clear();
                *self.app_state.backend_state.lock().unwrap() = "Offline".to_string();
                self.current_tick = VERSION_TICK - 1;
                self.persist_setting_value(
                    "inference_backend",
                    serde_json::to_value(backend)
                        .unwrap_or_else(|_| serde_json::Value::String("ollama".to_string())),
                );
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

            Message::ToggleShowTokensPerSecond => {
                self.show_tokens_per_second = !self.show_tokens_per_second;
                self.persist_boolean_setting("show_tokens_per_second", self.show_tokens_per_second);
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
                self.user_information.text_size = n.clamp(MIN_TEXT_SIZE, MAX_TEXT_SIZE);
                self.persist_setting_value(
                    "text_size",
                    serde_json::Value::from(self.user_information.text_size as f64),
                );
                self.trigger_settings_feedback(SettingsFeedbackTarget::TextSize);
                Task::none()
            }

            Message::FontFamilyChange(font_family) => {
                self.user_information.font_family = font_family;
                match serde_json::to_value(font_family) {
                    Ok(value) => self.persist_setting_value("font_family", value),
                    Err(error) => self.set_debug_message(DebugMessage {
                        message: format!("Could not save the response font: {error}"),
                        is_error: true,
                    }),
                }
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

            Message::ToggleInfoPopupSetting => {
                self.show_info_popup = !self.show_info_popup;
                self.persist_boolean_setting("info_popup", self.show_info_popup);
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

            Message::CheckCode(scope, code_block_index, language) => {
                if !self.code_checking_enabled {
                    self.code_check_result = Some(DebugMessage {
                        message:
                            "Enable code checking in Advanced settings before running local tools."
                                .into(),
                        is_error: true,
                    });
                    return Task::none();
                }
                let Some(code) = self.code_block_for_scope(scope, code_block_index) else {
                    self.code_check_result = Some(DebugMessage {
                        message: "That code snippet is no longer available.".into(),
                        is_error: true,
                    });
                    return Task::none();
                };
                self.code_check_result = Some(DebugMessage {
                    message: format!("Checking {language} code…"),
                    is_error: false,
                });
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || check_code(language, code))
                            .await
                            .unwrap_or_else(|error| {
                                Err(format!("Code checker could not complete: {error}"))
                            })
                    },
                    Message::CodeChecked,
                )
            }

            Message::CodeChecked(result) => {
                self.code_check_result = Some(match result {
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

            Message::DismissCodeCheckResult => {
                self.code_check_result = None;
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
                if self
                    .profiles
                    .iter()
                    .any(|profile| profile.id != id && profile.name.eq_ignore_ascii_case(&name))
                {
                    self.set_debug_message(DebugMessage {
                        message: "A profile with that name already exists.".to_string(),
                        is_error: true,
                    });
                    return Task::none();
                }
                if let Some(profile) = self.profiles.iter_mut().find(|profile| profile.id == id) {
                    profile.name = name;
                    profile.user_name = self.profile_edit_user_name.trim().to_string();
                    profile.custom_instructions = self.profile_edit_instructions.trim().to_string();
                }
                self.editing_profile_id = None;
                self.persist_profiles();
                Task::none()
            }

            Message::DeleteProfile(id) => {
                if id == self.active_profile_id {
                    self.set_debug_message(DebugMessage {
                        message: "Switch to another profile before deleting this one.".to_string(),
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
                    || self.active_prompts.values().any(|job| job.profile_id == id);
                if still_has_chats {
                    self.set_debug_message(DebugMessage {
                        message: "That profile still has chats. Delete them first.".to_string(),
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
                if !self.user_information.backend.supports_model_install() {
                    self.set_debug_message(DebugMessage {
                        message: "Deploy OpenVINO models with OpenVINO Model Server, then refresh the model list here."
                            .to_string(),
                        is_error: false,
                    });
                    return Task::none();
                }
                let base_url = match backend_base_url(
                    InferenceBackend::Ollama,
                    self.user_information.active_connection(),
                ) {
                    Ok(url) => url,
                    Err(message) => {
                        self.set_debug_message(DebugMessage {
                            message,
                            is_error: true,
                        });
                        return Task::none();
                    }
                };
                Channels::send_request_to_channel(
                    Arc::clone(&self.channels.debug_channel),
                    DebugMessage {
                        message: format!("Installing model... {}", model_install),
                        is_error: false,
                    },
                );

                let ollama = Ollama::builder().url(base_url).build();
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
                self.user_information.thinking_levels = vec![ThinkingLevel::Off];
                // Reasoning support and accepted effort values vary by model. Do not carry an
                // effort setting across models while capability detection is still in flight.
                self.user_information.thinking_level = ThinkingLevel::Off;
                if self.user_information.backend == InferenceBackend::OpenVino {
                    // OVMS exposes reasoning through chat-template parameters. The
                    // exact accepted values remain model/template specific, so offer
                    // the common controls while leaving vision capability unknown.
                    self.user_information.thinking_levels = vec![
                        ThinkingLevel::Off,
                        ThinkingLevel::On,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                    ];
                    self.user_information.thinking_supported = Some(true);
                    self.user_information.vision_supported = None;
                    return Task::none();
                }
                let location = self.user_information.active_connection().clone();
                Task::perform(
                    async move {
                        let result =
                            match backend_api_url(InferenceBackend::Ollama, &location, "/api/show")
                            {
                                Ok(url) => reqwest::Client::new()
                                    .post(url)
                                    .json(&serde_json::json!({ "model": model }))
                                    .send()
                                    .await
                                    .ok(),
                                Err(_) => None,
                            };
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

            Message::InstallationPrompt => {
                open_url(self.user_information.backend.setup_url().to_string())
            }

            Message::ListPrompt => open_url(
                self.user_information
                    .backend
                    .model_catalog_url()
                    .to_string(),
            ),

            Message::CopyResponse(message_index) => {
                let input = self
                    .chat_messages_cache
                    .get(message_index)
                    .and_then(|message| {
                        if let Correspondence::Bot { text, .. } = message {
                            Some(text.clone())
                        } else {
                            None
                        }
                    });
                self.copy_text(input)
            }

            Message::CopyCode(scope, code_block_index) => {
                self.copy_code_block(scope, code_block_index)
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
        let setting_f32 = |key, default| {
            settings_hmap
                .get(key)
                .and_then(serde_json::Value::as_f64)
                .map(|value| value as f32)
                .unwrap_or(default)
        };
        let filtering = setting_bool("filtering", true);
        let dark_mode = setting_bool("dark_mode", true);
        let info_popup = setting_bool("info_popup", false);
        let fast_streaming = setting_bool("fast_streaming", true);
        let show_tokens_per_second = setting_bool("show_tokens_per_second", false);
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
        let tool_settings = settings_hmap
            .get("tools")
            .cloned()
            .and_then(|value| serde_json::from_value::<crate::tools::ToolSettings>(value).ok())
            .unwrap_or_default();
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
        let text_size =
            setting_f32("text_size", DEFAULT_TEXT_SIZE).clamp(MIN_TEXT_SIZE, MAX_TEXT_SIZE);
        let font_family = settings_hmap
            .get("font_family")
            .cloned()
            .and_then(|value| serde_json::from_value::<FontFamily>(value).ok())
            .unwrap_or_default();
        let language = match settings_hmap
            .get("language")
            .and_then(|value| value.as_str())
        {
            Some("spanish") => Language::Spanish,
            _ => Language::English,
        };
        let backend = settings_hmap
            .get("inference_backend")
            .cloned()
            .and_then(|value| serde_json::from_value::<InferenceBackend>(value).ok())
            .unwrap_or_default();
        let backend_connections = settings_hmap
            .get("backend_connections")
            .cloned()
            .and_then(|value| serde_json::from_value::<BackendConnections>(value).ok())
            .unwrap_or_default();
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
        // The remembered web-search choice for new chats is deliberately not
        // persisted, so every launch starts from OFF regardless of the global
        // setting. Restored chats keep their own saved per-chat value.
        let new_chat_web_search = false;
        let web_search_for_chat = restored_chat
            .as_ref()
            .and_then(|chat| chat.web_search_enabled)
            .unwrap_or(new_chat_web_search);
        let current_chat =
            restored_chat
                .as_ref()
                .map(SavedChat::to_current)
                .unwrap_or(CurrentChat {
                    chats: vec![],
                    messages: vec![],
                    bot_responding: false,
                });

        let (chat_notice_sender, chat_notice_receiver) = crossbeam_channel::unbounded();
        gui::set_dark_mode(dark_mode);

        Self {
            batch_tokens: 3,
            fast_streaming,
            show_tokens_per_second,
            chat_menu_open: true,
            config_drawer_open: false,
            chat_row_menu: None,
            sidebar_animation: 1.0,
            ui_motion: 0.0,
            page_reveal: 0.0,
            ui_layout,
            ui_resize_target: None,
            window_size: Size::new(1100.0, 800.0),
            temporary_chat: false,
            web_search_for_chat,
            new_chat_web_search,
            web_search_settings,
            tool_settings,
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
            code_check_result: None,
            chat_markdown_cache: Vec::new(),
            chat_messages_cache: Vec::new(),
            chat_thinking_cache: Vec::new(),
            chat_visible_text_cache: Vec::new(),
            chat_model_name_cache: Vec::new(),
            last_copied_text: None,
            last_copied_at: None,
            pending_images: Vec::new(),
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
            },
            user_information: UserInformation {
                chat_history: Arc::new(Mutex::new(current_chat)),
                current_chat_history_enabled,
                backend,
                backend_connections,
                model: None,
                thinking_level: ThinkingLevel::Off,
                thinking_levels: vec![ThinkingLevel::Off],
                thinking_supported: None,
                vision_supported: None,
                max_response_tokens,
                context_tokens,
                temperature: 7.0,
                text_size,
                font_family,
                language,
            },
            show_info_popup: info_popup,
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
                backend_state: Arc::new(Mutex::new("Offline".to_string())),
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
        .title("Locoryn")
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

    use crate::inference::{HostLocation, InferenceBackend, api_url, base_url};

    use super::{
        ActivePrompt, Correspondence, CurrentChat, FontFamily, GUIState, InferenceStreamLine,
        LEGACY_PROFILE_ID, Message, ModelCapabilities, Point, Profile, ProfileRegistry, Program,
        SavedChat, SettingsFeedbackTarget, Size, ThinkingLevel, ToolLoopProgress, UiResizeTarget,
        UserInformation, WebSearchSettings, WebSearchState, app_data_dir,
        assign_legacy_profile_ids, canonical_code_language, censor_text, chat_profile_id,
        compare_versions, conversation_context_prompt, decode_generation_line,
        decode_inference_stream_line, disabled_web_tool_message, ensure_legacy_profile,
        mask_live_code_blocks, model_capabilities, normalize_code_fence_languages,
        parse_live_markdown_items, parse_markdown_items, read_json_with_backup,
        remote_image_url_is_safe, sidecar_path, split_thinking_text, tokens_per_second,
        write_json_safely,
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
            tokens_per_second: Arc::new(Mutex::new(None)),
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
    fn ollama_address_supports_http_https_and_ipv6() {
        let https = HostLocation {
            protocol: "https://".into(),
            ip: "ollama.example.com".into(),
            port: "443".into(),
        };
        assert_eq!(
            api_url(InferenceBackend::Ollama, &https, "/api/chat").unwrap(),
            "https://ollama.example.com/api/chat"
        );

        let ipv6 = HostLocation {
            protocol: "http".into(),
            ip: "::1".into(),
            port: "11434".into(),
        };
        assert_eq!(
            base_url(InferenceBackend::Ollama, &ipv6).unwrap().as_str(),
            "http://[::1]:11434/"
        );
    }

    #[test]
    fn ollama_address_rejects_unsupported_schemes_and_bad_ports() {
        let invalid_scheme = HostLocation {
            protocol: "ftp".into(),
            ip: "ollama.example.com".into(),
            port: "21".into(),
        };
        assert!(base_url(InferenceBackend::Ollama, &invalid_scheme).is_err());

        let invalid_port = HostLocation {
            protocol: "https".into(),
            ip: "ollama.example.com".into(),
            port: "invalid".into(),
        };
        assert!(base_url(InferenceBackend::Ollama, &invalid_port).is_err());
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
    fn chat_appearance_changes_are_bounded_and_queued_for_persistence() {
        let mut program = Program::default();

        let _ = program.update(Message::UpdateTextSize(99.0));
        let _ = program.update(Message::FontFamilyChange(FontFamily::Serif));

        assert_eq!(program.user_information.text_size, 40.0);
        assert_eq!(program.user_information.font_family, FontFamily::Serif);
        assert_eq!(
            program.pending_settings.get("text_size"),
            Some(&serde_json::json!(40.0))
        );
        assert_eq!(
            program.pending_settings.get("font_family"),
            Some(&serde_json::json!("serif"))
        );
    }

    #[test]
    fn opening_info_manually_does_not_change_the_startup_preference() {
        let mut program = Program {
            show_info_popup: false,
            ..Program::default()
        };
        program.app_state.gui_state = GUIState::Main;

        let _ = program.update(Message::ToggleInfoPopup);

        assert!(program.app_state.gui_state == GUIState::InfoPopup);
        assert!(!program.show_info_popup);
        assert!(!program.pending_settings.contains_key("info_popup"));
    }

    #[test]
    fn deep_research_controls_are_collapsed_by_default() {
        let mut program = Program::default();
        assert!(!program.deep_research_controls_open);

        let _ = program.update(Message::ToggleDeepResearchControls);
        assert!(program.deep_research_controls_open);
    }

    #[test]
    fn compact_config_drawer_is_collapsed_by_default() {
        let mut program = Program::default();
        assert!(!program.config_drawer_open);

        drop(program.update(Message::ToggleConfigDrawer));
        assert!(program.config_drawer_open);
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
                tokens_per_second: None,
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
    fn code_copy_lookup_reads_only_the_requested_block() {
        let items =
            parse_markdown_items("```rust\nlet first = 1;\n```\n\n```python\nsecond = 2\n```");

        assert_eq!(
            Program::code_block_text(&items, 0).as_deref(),
            Some("let first = 1;\n")
        );
        assert_eq!(
            Program::code_block_text(&items, 1).as_deref(),
            Some("second = 2\n")
        );
        assert_eq!(Program::code_block_text(&items, 2), None);
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
    fn reads_vision_capabilities() {
        let details = serde_json::json!({
            "capabilities": ["completion", "thinking", "vision"]
        });
        assert_eq!(
            model_capabilities(&details),
            Some(ModelCapabilities {
                thinking_levels: vec![ThinkingLevel::Off, ThinkingLevel::On],
                vision: true,
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
    fn retains_openvino_usage_event_for_generation_speed() {
        let line = r#"data: {"choices":[],"usage":{"completion_tokens":120}}"#;
        let InferenceStreamLine::Chunk(chunk, reason) =
            decode_inference_stream_line(InferenceBackend::OpenVino, line).unwrap()
        else {
            panic!("expected an OpenVINO generation chunk");
        };

        assert_eq!(chunk.eval_count, Some(120));
        assert_eq!(chunk.eval_duration, None);
        assert!(!chunk.done);
        assert_eq!(reason, None);
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
                tokens_per_second: None,
                sources: Vec::new(),
                web_search_used: false,
            }],
            bot_responding: false,
        };

        Program::apply_response_metadata(&mut chat, 1, "new-model", 9, Some(42.0));

        assert!(matches!(
            &chat.messages[0],
            Correspondence::Bot {
                model: None,
                thinking_seconds: None,
                tokens_per_second: None,
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
                    tokens_per_second: None,
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
                    tokens_per_second: None,
                    sources: Vec::new(),
                    web_search_used: false,
                },
            ],
            bot_responding: false,
        };

        Program::apply_response_metadata(&mut chat, 2, "new-model", 9, Some(42.0));

        assert!(matches!(
            &chat.messages[0],
            Correspondence::Bot {
                model: None,
                thinking_seconds: None,
                tokens_per_second: None,
                ..
            }
        ));
        assert!(matches!(
            &chat.messages[2],
            Correspondence::Bot {
                model: Some(model),
                thinking_seconds: Some(9),
                tokens_per_second: Some(tps),
                ..
            } if model == "new-model" && (tps - 42.0).abs() < f32::EPSILON
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
            tokens_per_second: None,
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
    fn live_markdown_skips_code_highlighting_but_preserves_the_complete_code() {
        let input = "Before\n\n```rust\nfn main() {\n    println!(\"hello\");\n}\n```\n\nAfter";
        let (masked, _) = mask_live_code_blocks(input);
        assert!(!masked.contains("```"));
        assert!(
            !markdown::parse(&masked).any(|item| matches!(item, markdown::Item::CodeBlock { .. }))
        );

        let expected_code = parse_markdown_items(input)
            .into_iter()
            .find_map(|item| match item {
                markdown::Item::CodeBlock { code, .. } => Some(code),
                _ => None,
            })
            .unwrap();
        let live = parse_live_markdown_items(input);

        assert!(matches!(&live[0], markdown::Item::Paragraph(_)));
        assert!(matches!(live.last(), Some(markdown::Item::Paragraph(_))));
        assert!(live.iter().any(|item| matches!(
            item,
            markdown::Item::CodeBlock { code, lines, language: Some(language) }
                if code == &expected_code && lines.is_empty() && language == "rust"
        )));
    }

    #[test]
    fn live_markdown_preserves_an_unclosed_streaming_code_fence() {
        let live = parse_live_markdown_items("```csharp\nConsole.WriteLine(\"still streaming\");");

        assert!(matches!(
            live.as_slice(),
            [markdown::Item::CodeBlock { code, lines, language: Some(language) }]
                if code == "Console.WriteLine(\"still streaming\");" && lines.is_empty() && language == "cs"
        ));
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
        let mut chats = vec![test_saved_chat(
            "chat-1",
            "profile-9",
            "2026-01-01T00:00:00Z",
        )];

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
        assert!(
            program
                .saved_chats
                .iter()
                .all(|chat| chat.id != program.current_chat_id)
        );
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
            saved_chats: vec![test_saved_chat(
                "chat-1",
                "profile-1",
                "2026-01-01T00:00:00Z",
            )],
            ..Program::default()
        };

        drop(program.update(Message::DeleteProfile("profile-1".into())));
        assert!(
            program
                .profiles
                .iter()
                .any(|profile| profile.id == "profile-1")
        );

        // The active profile is protected as well.
        drop(program.update(Message::DeleteProfile(LEGACY_PROFILE_ID.into())));
        assert!(
            program
                .profiles
                .iter()
                .any(|profile| profile.id == LEGACY_PROFILE_ID)
        );

        program.saved_chats.clear();
        drop(program.update(Message::DeleteProfile("profile-1".into())));
        assert!(
            program
                .profiles
                .iter()
                .all(|profile| profile.id != "profile-1")
        );
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

        assert!(
            prompt.contains("The user's name is Logan. Use this name when addressing the user.")
        );
    }

    #[test]
    fn conversation_context_names_the_user_the_profile_provides() {
        let prompt = conversation_context_prompt("User: Hello there", "Logan", "What is my name?");

        assert!(prompt.contains(
            "a conversation between an AI language model and Logan. You are the AI language model:"
        ));
        assert!(prompt.contains("User: Hello there"));
        assert!(prompt.contains("What is my name?"));
    }

    #[test]
    fn conversation_context_falls_back_to_anonymous_user_without_a_name() {
        let prompt = conversation_context_prompt("User: Hello there", "   ", "Hi");

        assert!(prompt.contains(
            "a conversation between an AI language model and a User. You are the AI language model:"
        ));
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

        let ids: Vec<&str> = program
            .saved_chats
            .iter()
            .map(|chat| chat.id.as_str())
            .collect();
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

        let ids: Vec<&str> = program
            .saved_chats
            .iter()
            .map(|chat| chat.id.as_str())
            .collect();
        assert_eq!(ids, vec!["chat-a", "chat-b", "chat-c"]);
        assert!(program.saved_chats.iter().all(|chat| !chat.pinned));
    }

    #[test]
    fn tokens_per_second_uses_ollama_statistics() {
        // 100 tokens in 2.5 seconds -> 40 tokens/s.
        let tps = tokens_per_second(Some(100), Some(2_500_000_000)).unwrap();
        assert!((tps - 40.0).abs() < 0.01);

        // Missing or empty stats never produce a rate.
        assert_eq!(tokens_per_second(None, Some(2_500_000_000)), None);
        assert_eq!(tokens_per_second(Some(100), None), None);
        assert_eq!(tokens_per_second(Some(0), Some(2_500_000_000)), None);
        assert_eq!(tokens_per_second(Some(100), Some(0)), None);
    }

    #[test]
    fn new_chats_start_with_web_search_off_even_when_globally_enabled() {
        let mut program = Program {
            web_search_settings: WebSearchSettings {
                enabled: true,
                ..WebSearchSettings::default()
            },
            ..Program::default()
        };

        drop(program.update(Message::NewChat));

        assert!(!program.web_search_for_chat);
    }

    #[test]
    fn chat_web_search_toggle_becomes_the_default_for_new_chats() {
        let mut program = Program::default();

        // Turn web search on for the chat; new chats now follow that choice.
        drop(program.update(Message::ToggleChatWebSearch));
        assert!(program.web_search_for_chat);
        drop(program.update(Message::NewChat));
        assert!(program.web_search_for_chat);

        // Turn it off again; new chats follow it back down.
        drop(program.update(Message::ToggleChatWebSearch));
        assert!(!program.web_search_for_chat);
        drop(program.update(Message::NewChat));
        assert!(!program.web_search_for_chat);
    }

    #[test]
    fn remembered_chat_web_search_choice_is_not_restored_after_restart() {
        let mut program = Program::default();
        drop(program.update(Message::ToggleChatWebSearch));
        assert!(program.new_chat_web_search);

        // Simulates a restart: the remembered choice lives in memory only.
        let restarted = Program::default();
        assert!(!restarted.new_chat_web_search);
        assert!(!restarted.web_search_for_chat);
    }

    #[test]
    fn show_tokens_per_second_toggle_flips_and_defaults_off() {
        let mut program = Program::default();
        assert!(!program.show_tokens_per_second);

        drop(program.update(Message::ToggleShowTokensPerSecond));
        assert!(program.show_tokens_per_second);

        drop(program.update(Message::ToggleShowTokensPerSecond));
        assert!(!program.show_tokens_per_second);
    }
}
