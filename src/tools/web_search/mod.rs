use std::{
    fmt,
    net::{IpAddr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use async_trait::async_trait;
use crossbeam_channel::Sender;
use reqwest::{Client, StatusCode, header};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::inference::{EncodedImage, GenerationDetails, InferenceBackend};

mod fetch;
mod providers;
mod tool_loop;

use fetch::*;

#[cfg(test)]
use providers::*;
#[cfg(test)]
use tool_loop::*;

pub(crate) use fetch::send_inference_request_with_retry;
pub use fetch::validate_public_url;
pub use providers::{BraveSearchProvider, ExaSearchProvider, TavilySearchProvider};
#[allow(unused_imports)]
pub use tool_loop::ToolLoopResponse;
pub use tool_loop::{ToolLoopProgress, ToolLoopRequest, run_tool_loop};

#[cfg(test)]
mod tests;

pub const DEFAULT_RESULT_LIMIT: usize = 5;
pub const MAX_RESULT_LIMIT: usize = 10;
const DEFAULT_SEARCHES_PER_MESSAGE: usize = 1;
const DEFAULT_PAGES_PER_MESSAGE: usize = 2;
pub const DEFAULT_MIN_FOLLOW_UP_SEARCHES: usize = 3;
pub const DEFAULT_MAX_SEARCHES_PER_MESSAGE: usize = 6;
pub const DEFAULT_MIN_CROSS_REFERENCE_PAGES: usize = 2;
pub const DEFAULT_MAX_PAGES_PER_MESSAGE: usize = 6;
pub const DEFAULT_TOOL_ITERATION_LIMIT: usize = 15;
pub const MAX_CONFIGURABLE_SEARCHES: usize = 20;
pub const MAX_CONFIGURABLE_PAGES: usize = 20;
pub const MAX_CONFIGURABLE_TOOL_ITERATIONS: usize = 64;
pub const MAX_CUSTOM_RESEARCH_INSTRUCTIONS_CHARS: usize = 2_000;
const MAX_STALLED_RESEARCH_REMINDERS: usize = 2;
#[cfg(test)]
pub const MAX_TOOL_ITERATIONS: usize = DEFAULT_MAX_SEARCHES_PER_MESSAGE
    + DEFAULT_MAX_PAGES_PER_MESSAGE
    + MAX_STALLED_RESEARCH_REMINDERS
    + 1;
pub const MAX_PAGE_BYTES: usize = 512 * 1024;
const MAX_PAGE_TEXT_CHARS: usize = 24 * 1024;
const MAX_REDIRECTS: usize = 5;
const MIN_SEARCH_INTERVAL: Duration = Duration::from_millis(1_100);
const MAX_RATE_LIMIT_RETRIES: usize = 2;
static BRAVE_SEARCH_PACER: OnceLock<tokio::sync::Mutex<Option<Instant>>> = OnceLock::new();
static TAVILY_SEARCH_PACER: OnceLock<tokio::sync::Mutex<Option<Instant>>> = OnceLock::new();
static EXA_SEARCH_PACER: OnceLock<tokio::sync::Mutex<Option<Instant>>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchProviderKind {
    #[default]
    Brave,
    Tavily,
    Exa,
}

impl WebSearchProviderKind {
    pub const ALL: [Self; 3] = [Self::Brave, Self::Tavily, Self::Exa];

    pub fn api_key_environment_variable(self) -> &'static str {
        match self {
            Self::Brave => "BRAVE_SEARCH_API_KEY",
            Self::Tavily => "TAVILY_API_KEY",
            Self::Exa => "EXA_API_KEY",
        }
    }

    pub fn api_key_placeholder(self) -> &'static str {
        match self {
            Self::Brave => "Brave Search API key",
            Self::Tavily => "Tavily API key",
            Self::Exa => "Exa API key",
        }
    }

    pub fn api_key_help(self) -> &'static str {
        match self {
            Self::Brave => {
                "Prefer BRAVE_SEARCH_API_KEY for secret storage. A key entered here is stored in the local settings file and never printed in logs."
            }
            Self::Tavily => {
                "Prefer TAVILY_API_KEY for secret storage. Tavily support is experimental; a key entered here is stored locally and never printed in logs."
            }
            Self::Exa => {
                "Prefer EXA_API_KEY for secret storage. Exa support is experimental; a key entered here is stored locally and never printed in logs."
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WebSearchFreshness {
    #[default]
    Any,
    Day,
    Week,
    Month,
    Year,
}

impl WebSearchFreshness {
    fn from_tool_value(value: Option<&serde_json::Value>) -> Result<Self, WebSearchError> {
        match value {
            None => Ok(Self::Any),
            Some(value) => match value.as_str() {
                Some("any") => Ok(Self::Any),
                Some("day") => Ok(Self::Day),
                Some("week") => Ok(Self::Week),
                Some("month") => Ok(Self::Month),
                Some("year") => Ok(Self::Year),
                _ => Err(WebSearchError::InvalidToolCall),
            },
        }
    }

    fn brave_value(self) -> Option<&'static str> {
        match self {
            Self::Any => None,
            Self::Day => Some("pd"),
            Self::Week => Some("pw"),
            Self::Month => Some("pm"),
            Self::Year => Some("py"),
        }
    }

    fn tavily_value(self) -> Option<&'static str> {
        match self {
            Self::Any => None,
            Self::Day => Some("day"),
            Self::Week => Some("week"),
            Self::Month => Some("month"),
            Self::Year => Some("year"),
        }
    }

    fn exa_lookback_days(self) -> Option<i64> {
        match self {
            Self::Any => None,
            Self::Day => Some(1),
            Self::Week => Some(7),
            Self::Month => Some(31),
            Self::Year => Some(365),
        }
    }

    fn tool_value(self) -> &'static str {
        match self {
            Self::Any => "any",
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
        }
    }
}

impl fmt::Display for WebSearchProviderKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Brave => "Brave Search",
            Self::Tavily => "Tavily (experimental)",
            Self::Exa => "Exa (experimental)",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct WebSearchSettings {
    pub enabled: bool,
    /// Opts into multi-query, multi-source research during the same response.
    pub allow_multiple_searches: bool,
    pub provider: WebSearchProviderKind,
    /// Brave's saved key. The original field name is retained so existing
    /// settings files continue to load without migration.
    pub api_key: Option<String>,
    pub tavily_api_key: Option<String>,
    pub exa_api_key: Option<String>,
    pub result_limit: usize,
    pub request_timeout_seconds: u64,
    pub maximum_searches: usize,
    pub maximum_page_fetches: usize,
    pub minimum_successful_searches: usize,
    pub minimum_independent_pages: usize,
    pub tool_iteration_limit: usize,
    pub custom_research_instructions: String,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            allow_multiple_searches: false,
            provider: WebSearchProviderKind::Brave,
            api_key: None,
            tavily_api_key: None,
            exa_api_key: None,
            result_limit: DEFAULT_RESULT_LIMIT,
            request_timeout_seconds: 15,
            maximum_searches: DEFAULT_MAX_SEARCHES_PER_MESSAGE,
            maximum_page_fetches: DEFAULT_MAX_PAGES_PER_MESSAGE,
            minimum_successful_searches: DEFAULT_MIN_FOLLOW_UP_SEARCHES,
            minimum_independent_pages: DEFAULT_MIN_CROSS_REFERENCE_PAGES,
            tool_iteration_limit: DEFAULT_TOOL_ITERATION_LIMIT,
            custom_research_instructions: String::new(),
        }
    }
}

impl WebSearchSettings {
    pub fn normalized(mut self) -> Self {
        self.api_key = normalize_api_key(self.api_key);
        self.tavily_api_key = normalize_api_key(self.tavily_api_key);
        self.exa_api_key = normalize_api_key(self.exa_api_key);
        self.result_limit = self.result_limit.clamp(1, MAX_RESULT_LIMIT);
        self.request_timeout_seconds = self.request_timeout_seconds.clamp(3, 60);
        self.maximum_searches = self.maximum_searches.clamp(1, MAX_CONFIGURABLE_SEARCHES);
        self.maximum_page_fetches = self.maximum_page_fetches.min(MAX_CONFIGURABLE_PAGES);
        self.minimum_successful_searches = self
            .minimum_successful_searches
            .clamp(1, self.maximum_searches);
        self.minimum_independent_pages = self
            .minimum_independent_pages
            .min(self.maximum_page_fetches);
        self.tool_iteration_limit = self
            .tool_iteration_limit
            .clamp(2, MAX_CONFIGURABLE_TOOL_ITERATIONS);
        self.custom_research_instructions = self
            .custom_research_instructions
            .trim()
            .chars()
            .take(MAX_CUSTOM_RESEARCH_INSTRUCTIONS_CHARS)
            .collect();
        self
    }

    pub fn selected_api_key(&self) -> Option<&str> {
        self.api_key_for(self.provider)
    }

    pub fn resolved_api_key(&self) -> Option<String> {
        self.resolved_api_key_for(self.provider)
    }

    fn api_key_for(&self, provider: WebSearchProviderKind) -> Option<&str> {
        match provider {
            WebSearchProviderKind::Brave => self.api_key.as_deref(),
            WebSearchProviderKind::Tavily => self.tavily_api_key.as_deref(),
            WebSearchProviderKind::Exa => self.exa_api_key.as_deref(),
        }
    }

    fn resolved_api_key_for(&self, provider: WebSearchProviderKind) -> Option<String> {
        self.api_key_for(provider)
            .map(str::to_owned)
            .or_else(|| std::env::var(provider.api_key_environment_variable()).ok())
            .and_then(|key| normalize_api_key(Some(key)))
    }

    pub fn set_selected_api_key(&mut self, api_key: Option<String>) {
        let key = normalize_api_key(api_key);
        match self.provider {
            WebSearchProviderKind::Brave => self.api_key = key,
            WebSearchProviderKind::Tavily => self.tavily_api_key = key,
            WebSearchProviderKind::Exa => self.exa_api_key = key,
        }
    }
}

fn normalize_api_key(api_key: Option<String>) -> Option<String> {
    api_key.and_then(|key| {
        let key = key.trim().to_string();
        (!key.is_empty()).then_some(key)
    })
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebSearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebPageContent {
    pub url: String,
    pub title: Option<String>,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct WebSource {
    pub title: String,
    pub url: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum WebSearchState {
    #[default]
    Idle,
    Searching {
        query: String,
        websites: Vec<WebSource>,
    },
    Results {
        query: String,
        websites: Vec<WebSource>,
    },
    Fetching {
        url: String,
        query: String,
        websites: Vec<WebSource>,
    },
    /// The web tools have returned control to the model. Research context stays
    /// attached so the UI can keep the accumulated results visible while the
    /// next action or final answer streams.
    Synthesizing {
        thinking: String,
        query: String,
        websites: Vec<WebSource>,
    },
    Completed,
    Failed {
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WebSearchError {
    Disabled,
    MissingApiKey,
    InvalidUrl,
    UnsupportedScheme,
    UnsafeAddress,
    TooManyRedirects,
    ResponseTooLarge,
    UnsupportedContentType,
    Unauthorized,
    RateLimited,
    Timeout,
    EmptyResults,
    InvalidToolCall,
    ModelToolsUnsupported,
    /// Native function calling was accepted by the server, but the model
    /// returned neither a tool call nor a usable answer. Locoryn can recover
    /// by performing one compact search itself before a no-tools synthesis.
    WebSearchNotPerformed,
    /// Kept distinct from ordinary inference failures so NPU deployments with
    /// a small `max_prompt_len` can retry with a compact, no-tools request.
    ContextLengthExceeded(String),
    InferenceUnavailable(String),
    ProviderUnavailable(String),
    Cancelled,
}

impl WebSearchError {
    pub fn user_message(&self) -> &'static str {
        match self {
            Self::Disabled => "Web search is disabled. Enable it in Settings or for this chat.",
            Self::MissingApiKey => "Add a search API key in Settings.",
            Self::InvalidUrl => "The requested webpage URL is invalid.",
            Self::UnsupportedScheme => "Only HTTP and HTTPS webpages can be opened.",
            Self::UnsafeAddress => "Local and private-network webpages are blocked.",
            Self::TooManyRedirects => "The webpage redirected too many times.",
            Self::ResponseTooLarge => "The webpage is too large to read safely.",
            Self::UnsupportedContentType => "The webpage is not readable text or HTML.",
            Self::Unauthorized => "The search API key was rejected.",
            Self::RateLimited => "The search provider rate limit was reached.",
            Self::Timeout => "The web request timed out.",
            Self::EmptyResults => "The search returned no results.",
            Self::InvalidToolCall => "The model requested web access with invalid arguments.",
            Self::ModelToolsUnsupported => {
                "The selected model or inference server does not support tool calling."
            }
            Self::WebSearchNotPerformed => "The model did not perform the requested web search.",
            Self::ContextLengthExceeded(_) => {
                "The inference request exceeded the model server's prompt-length limit. For OpenVINO on NPU, increase --max_prompt_len or shorten the active chat context."
            }
            Self::InferenceUnavailable(_) => {
                "The inference backend could not complete the tool-enabled response."
            }
            Self::ProviderUnavailable(_) => "The web-search provider is unavailable.",
            Self::Cancelled => "Web search was cancelled.",
        }
    }

    pub fn diagnostic(&self, api_key: Option<&str>) -> String {
        let detail = match self {
            Self::InferenceUnavailable(detail) | Self::ContextLengthExceeded(detail) => {
                format!("inference backend unavailable: {detail}")
            }
            Self::ProviderUnavailable(detail) => {
                format!("provider unavailable: {detail}")
            }
            other => format!("{other:?}"),
        };
        redact_secret(&detail, api_key)
    }

    pub fn detailed_user_message(&self, api_key: Option<&str>) -> String {
        let detail = match self {
            Self::InferenceUnavailable(detail)
            | Self::ProviderUnavailable(detail)
            | Self::ContextLengthExceeded(detail) => redact_secret(detail, api_key)
                .trim()
                .chars()
                .take(320)
                .collect::<String>(),
            _ => String::new(),
        };
        if detail.is_empty() {
            self.user_message().to_string()
        } else {
            format!("{} Details: {detail}", self.user_message())
        }
    }
}

impl fmt::Display for WebSearchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.user_message())
    }
}

impl std::error::Error for WebSearchError {}

pub fn redact_secret(text: &str, secret: Option<&str>) -> String {
    match secret.filter(|secret| !secret.is_empty()) {
        Some(secret) => text.replace(secret, "<redacted>"),
        None => text.to_string(),
    }
}

#[async_trait]
pub trait WebSearchProvider: Send + Sync {
    async fn search(
        &self,
        query: &str,
        limit: usize,
        freshness: WebSearchFreshness,
    ) -> Result<Vec<WebSearchResult>, WebSearchError>;

    async fn fetch_page(&self, url: &str) -> Result<WebPageContent, WebSearchError>;
}

pub fn create_search_provider(
    settings: &WebSearchSettings,
) -> Result<Arc<dyn WebSearchProvider>, WebSearchError> {
    let normalized = settings.clone().normalized();
    match normalized.provider {
        WebSearchProviderKind::Brave => Ok(Arc::new(BraveSearchProvider::new(&normalized)?)),
        WebSearchProviderKind::Tavily => Ok(Arc::new(TavilySearchProvider::new(&normalized)?)),
        WebSearchProviderKind::Exa => Ok(Arc::new(ExaSearchProvider::new(&normalized)?)),
    }
}

fn provider_api_key(
    settings: &WebSearchSettings,
    provider: WebSearchProviderKind,
) -> Result<String, WebSearchError> {
    settings
        .resolved_api_key_for(provider)
        .ok_or(WebSearchError::MissingApiKey)
}

fn provider_http_client(settings: &WebSearchSettings) -> Result<Client, WebSearchError> {
    Client::builder()
        .timeout(Duration::from_secs(settings.request_timeout_seconds))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("locoryn/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| WebSearchError::ProviderUnavailable(error.to_string()))
}

async fn wait_for_search_slot(pacer: &'static OnceLock<tokio::sync::Mutex<Option<Instant>>>) {
    let mut last_search_at = pacer
        .get_or_init(|| tokio::sync::Mutex::new(None))
        .lock()
        .await;
    if let Some(last_search_at) = *last_search_at {
        let elapsed = last_search_at.elapsed();
        if elapsed < MIN_SEARCH_INTERVAL {
            tokio::time::sleep(MIN_SEARCH_INTERVAL - elapsed).await;
        }
    }
    *last_search_at = Some(Instant::now());
}

async fn send_search_request_with_retry(
    request: reqwest::RequestBuilder,
    pacer: &'static OnceLock<tokio::sync::Mutex<Option<Instant>>>,
) -> Result<reqwest::Response, WebSearchError> {
    let mut attempt = 0;
    loop {
        wait_for_search_slot(pacer).await;
        let request = request.try_clone().ok_or_else(|| {
            WebSearchError::ProviderUnavailable("search request could not be retried".into())
        })?;
        let response = match request.send().await {
            Ok(response) => response,
            Err(error)
                if attempt < MAX_RATE_LIMIT_RETRIES
                    && (error.is_timeout() || error.is_connect()) =>
            {
                let delay = Duration::from_secs(1_u64 << attempt.min(3));
                attempt += 1;
                tokio::time::sleep(delay).await;
                continue;
            }
            Err(error) => return Err(map_reqwest_error(error)),
        };
        if !is_transient_status(response.status()) || attempt >= MAX_RATE_LIMIT_RETRIES {
            return Ok(response);
        }
        let retry_after = retry_after_delay(&response, attempt);
        attempt += 1;
        tokio::time::sleep(retry_after).await;
    }
}
