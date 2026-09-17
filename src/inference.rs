use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InferenceBackend {
    #[default]
    Ollama,
    #[serde(
        rename = "openvino",
        alias = "open-vino",
        alias = "openvino-model-server"
    )]
    OpenVino,
}

impl InferenceBackend {
    pub const ALL: [Self; 2] = [Self::Ollama, Self::OpenVino];

    pub fn server_name(self) -> &'static str {
        match self {
            Self::Ollama => "Ollama",
            Self::OpenVino => "OpenVINO Model Server",
        }
    }

    pub fn default_location(self) -> HostLocation {
        match self {
            Self::Ollama => HostLocation::new("http", "127.0.0.1", "11434"),
            Self::OpenVino => HostLocation::new("http", "127.0.0.1", "8000"),
        }
    }

    pub fn status_path(self) -> &'static str {
        match self {
            Self::Ollama => "/api/version",
            Self::OpenVino => "/v3/models",
        }
    }

    pub fn models_path(self) -> &'static str {
        match self {
            Self::Ollama => "/api/tags",
            Self::OpenVino => "/v3/models",
        }
    }

    pub fn chat_path(self) -> &'static str {
        match self {
            Self::Ollama => "/api/chat",
            Self::OpenVino => "/v3/chat/completions",
        }
    }

    pub fn supports_model_install(self) -> bool {
        self == Self::Ollama
    }

    pub fn setup_url(self) -> &'static str {
        match self {
            Self::Ollama => "https://ollama.com/download",
            Self::OpenVino => {
                "https://docs.openvino.ai/2026/model-server/ovms_docs_serving_model.html"
            }
        }
    }

    pub fn model_catalog_url(self) -> &'static str {
        match self {
            Self::Ollama => "https://ollama.com/search",
            Self::OpenVino => "https://huggingface.co/OpenVINO",
        }
    }
}

impl fmt::Display for InferenceBackend {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Ollama => "Ollama",
            Self::OpenVino => "OpenVINO",
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HostLocation {
    /// Preserve the text as entered, including incomplete edits. None means
    /// this connection was saved by a version using the three legacy fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    // Retained so older Locoryn versions can still read saved connections.
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub ip: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub models_endpoint: String,
    #[serde(default)]
    pub chat_endpoint: String,
}

impl HostLocation {
    pub fn new(protocol: &str, ip: &str, port: &str) -> Self {
        Self {
            address: None,
            protocol: protocol.to_string(),
            ip: ip.to_string(),
            port: port.to_string(),
            models_endpoint: String::new(),
            chat_endpoint: String::new(),
        }
    }

    pub fn address(&self) -> String {
        self.address.clone().unwrap_or_else(|| {
            let protocol = self
                .protocol
                .trim()
                .trim_end_matches("://")
                .to_ascii_lowercase();
            let host = self.ip.trim();
            let host = if host.contains(':') && !host.starts_with('[') {
                format!("[{host}]")
            } else {
                host.to_string()
            };
            format!("{protocol}://{host}:{}", self.port.trim())
        })
    }

    pub fn set_address(&mut self, address: String) {
        // Keep the last usable legacy address while a new URL is being typed.
        if let Ok(url) = parse_address(&address) {
            self.protocol = url.scheme().to_string();
            self.ip = url.host_str().unwrap_or_default().to_string();
            self.port = url.port_or_known_default().unwrap_or_default().to_string();
        }
        self.address = Some(address);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct BackendConnections {
    pub ollama: HostLocation,
    pub openvino: HostLocation,
}

impl Default for BackendConnections {
    fn default() -> Self {
        Self {
            ollama: InferenceBackend::Ollama.default_location(),
            openvino: InferenceBackend::OpenVino.default_location(),
        }
    }
}

impl BackendConnections {
    pub fn active(&self, backend: InferenceBackend) -> &HostLocation {
        match backend {
            InferenceBackend::Ollama => &self.ollama,
            InferenceBackend::OpenVino => &self.openvino,
        }
    }

    pub fn active_mut(&mut self, backend: InferenceBackend) -> &mut HostLocation {
        match backend {
            InferenceBackend::Ollama => &mut self.ollama,
            InferenceBackend::OpenVino => &mut self.openvino,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodedImage {
    pub mime_type: String,
    pub data: String,
}

/// Durable generation measurements shown beside the output-token rate. Every
/// field is optional because backend APIs expose different subsets. Durations
/// are stored in milliseconds to keep saved chats readable and stable across
/// platforms.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default)]
pub struct GenerationDetails {
    pub prompt_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub prompt_duration_ms: Option<u64>,
    pub generation_duration_ms: Option<u64>,
    pub time_to_first_token_ms: Option<u64>,
    /// End-to-end time from Send until the response is reconciled. For web
    /// requests this deliberately includes searching and page retrieval.
    pub response_duration_ms: Option<u64>,
    /// Backend-reported total when available (currently Ollama). This can be
    /// slightly shorter than the end-to-end response duration above.
    pub backend_total_duration_ms: Option<u64>,
    pub load_duration_ms: Option<u64>,
    /// True when generation duration (and therefore tok/s) was measured by the
    /// client because the backend supplied a token count without a duration.
    pub generation_duration_locally_measured: bool,
}

impl GenerationDetails {
    pub fn is_empty(self) -> bool {
        self.prompt_tokens.is_none()
            && self.output_tokens.is_none()
            && self.prompt_duration_ms.is_none()
            && self.generation_duration_ms.is_none()
            && self.time_to_first_token_ms.is_none()
            && self.response_duration_ms.is_none()
            && self.backend_total_duration_ms.is_none()
            && self.load_duration_ms.is_none()
    }
}

pub fn base_url(backend: InferenceBackend, location: &HostLocation) -> Result<url::Url, String> {
    let server = backend.server_name();
    if let Some(address) = &location.address {
        return parse_address(address).map_err(|error| format!("{server}: {error}"));
    }
    let scheme = location
        .protocol
        .trim()
        .trim_end_matches("://")
        .to_ascii_lowercase();
    if !matches!(scheme.as_str(), "http" | "https") {
        return Err(format!("{server} protocol must be http or https."));
    }

    let host = location.ip.trim();
    if host.is_empty() {
        return Err(format!("Enter a {server} hostname or IP address."));
    }
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    let port = location
        .port
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("{server} port must be a number from 1 to 65535."))?;
    if port == 0 {
        return Err(format!("{server} port must be a number from 1 to 65535."));
    }

    let mut url = url::Url::parse(&format!("{scheme}://{host}"))
        .map_err(|_| format!("Enter a valid {server} hostname or IP address."))?;
    url.set_port(Some(port))
        .map_err(|_| format!("Enter a valid {server} hostname or IP address."))?;
    Ok(url)
}

fn parse_address(address: &str) -> Result<url::Url, String> {
    let mut url = url::Url::parse(address.trim())
        .map_err(|_| "Enter a full server URL, including http:// or https://.".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("The server URL must start with http:// or https://.".to_string());
    }
    if url.port() == Some(0) {
        return Err("The server port must be a number from 1 to 65535.".to_string());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("Enter a server base URL without a query string or fragment.".to_string());
    }
    // The Ollama client appends API paths directly to this base URL.
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}

pub fn api_url(
    backend: InferenceBackend,
    location: &HostLocation,
    path: &str,
) -> Result<String, String> {
    let mut url = base_url(backend, location)?;
    let custom_path = if backend == InferenceBackend::OpenVino {
        if path == backend.models_path() {
            location.models_endpoint.trim()
        } else if path == backend.chat_path() {
            location.chat_endpoint.trim()
        } else {
            ""
        }
    } else {
        ""
    };
    let path = if custom_path.is_empty() {
        path
    } else {
        custom_path
    };
    url.set_path(&format!(
        "{}/{}",
        url.path().trim_end_matches('/'),
        path.trim_start_matches('/')
    ));
    Ok(url.into())
}

pub fn model_names(
    backend: InferenceBackend,
    value: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let entries = match backend {
        InferenceBackend::Ollama => value.get("models"),
        InferenceBackend::OpenVino => value.get("data"),
    }
    .and_then(serde_json::Value::as_array)
    .ok_or_else(|| format!("{} returned an invalid model list.", backend.server_name()))?;

    let key = match backend {
        InferenceBackend::Ollama => "name",
        InferenceBackend::OpenVino => "id",
    };
    let mut names = entries
        .iter()
        .filter_map(|entry| entry.get(key).and_then(serde_json::Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    Ok(names)
}

pub fn user_message(
    backend: InferenceBackend,
    prompt: String,
    images: &[EncodedImage],
) -> serde_json::Value {
    if images.is_empty() {
        return serde_json::json!({"role": "user", "content": prompt});
    }

    match backend {
        InferenceBackend::Ollama => serde_json::json!({
            "role": "user",
            "content": prompt,
            "images": images.iter().map(|image| image.data.clone()).collect::<Vec<_>>(),
        }),
        InferenceBackend::OpenVino => {
            let mut content = vec![serde_json::json!({"type": "text", "text": prompt})];
            content.extend(images.iter().map(|image| {
                serde_json::json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:{};base64,{}", image.mime_type, image.data),
                    }
                })
            }));
            serde_json::json!({"role": "user", "content": content})
        }
    }
}

fn apply_openvino_thinking(body: &mut serde_json::Value, thinking: &serde_json::Value) {
    let kwargs = match thinking {
        serde_json::Value::Bool(enabled) => {
            serde_json::json!({"enable_thinking": enabled})
        }
        serde_json::Value::String(level) => serde_json::json!({
            "enable_thinking": true,
            "reasoning_effort": level,
        }),
        _ => return,
    };
    body["chat_template_kwargs"] = kwargs;
}

#[allow(clippy::too_many_arguments)]
pub fn chat_request_body(
    backend: InferenceBackend,
    model: &str,
    messages: &[serde_json::Value],
    tools: Option<&serde_json::Value>,
    thinking: &serde_json::Value,
    temperature: f32,
    top_p: f32,
    top_k: u32,
    context_tokens: u32,
    max_response_tokens: u32,
) -> serde_json::Value {
    let mut body = match backend {
        InferenceBackend::Ollama => serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "think": thinking,
            "options": {
                "temperature": temperature,
                "top_p": top_p,
                "top_k": top_k,
                "num_ctx": context_tokens,
                "num_predict": max_response_tokens,
            }
        }),
        InferenceBackend::OpenVino => serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "stream_options": {"include_usage": true},
            "temperature": temperature,
                "top_p": top_p,
                "top_k": top_k,
            "max_tokens": max_response_tokens,
        }),
    };
    if backend == InferenceBackend::OpenVino {
        apply_openvino_thinking(&mut body, thinking);
    }
    if let Some(tools) = tools {
        body["tools"] = tools.clone();
        if backend == InferenceBackend::OpenVino {
            body["tool_choice"] = serde_json::Value::String("auto".to_string());
        }
    }
    body
}

#[allow(clippy::too_many_arguments)]
pub fn direct_request_body(
    backend: InferenceBackend,
    model: &str,
    prompt: String,
    system_prompt: String,
    images: &[EncodedImage],
    thinking: &serde_json::Value,
    temperature: f32,
    top_p: f32,
    top_k: u32,
    context_tokens: u32,
    max_response_tokens: u32,
) -> serde_json::Value {
    match backend {
        InferenceBackend::Ollama => {
            let mut body = serde_json::json!({
                "model": model,
                "prompt": prompt,
                "system": system_prompt,
                "stream": true,
                "think": thinking,
                "options": {
                    "temperature": temperature,
                "top_p": top_p,
                "top_k": top_k,
                    "num_ctx": context_tokens,
                    "num_predict": max_response_tokens,
                }
            });
            if !images.is_empty() {
                body["images"] = serde_json::json!(
                    images
                        .iter()
                        .map(|image| image.data.clone())
                        .collect::<Vec<_>>()
                );
            }
            body
        }
        InferenceBackend::OpenVino => {
            let messages = vec![
                serde_json::json!({"role": "system", "content": system_prompt}),
                user_message(backend, prompt, images),
            ];
            chat_request_body(
                backend,
                model,
                &messages,
                None,
                thinking,
                temperature,
                top_p,
                top_k,
                context_tokens,
                max_response_tokens,
            )
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpenAiStreamEvent {
    pub role: Option<String>,
    pub content: String,
    pub reasoning: String,
    pub tool_calls: Vec<serde_json::Value>,
    pub finish_reason: Option<String>,
    pub prompt_tokens: Option<u64>,
    pub completion_tokens: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum OpenAiStreamLine {
    Event(OpenAiStreamEvent),
    Done,
    Ignore,
}

pub fn response_error_detail(value: &serde_json::Value) -> Option<String> {
    let error = value.get("error")?;
    error
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .or_else(|| Some(error.to_string()))
}

pub fn decode_openai_stream_line(input: &str) -> Result<OpenAiStreamLine, String> {
    let line = input.trim();
    if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
        return Ok(OpenAiStreamLine::Ignore);
    }
    let line = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
    if line.is_empty() {
        return Ok(OpenAiStreamLine::Ignore);
    }
    if line == "[DONE]" {
        return Ok(OpenAiStreamLine::Done);
    }

    let value = serde_json::from_str::<serde_json::Value>(line)
        .map_err(|error| format!("invalid OpenAI-compatible stream event: {error}"))?;
    if let Some(error) = response_error_detail(&value) {
        return Err(error);
    }

    let mut event = OpenAiStreamEvent {
        prompt_tokens: value
            .pointer("/usage/prompt_tokens")
            .and_then(serde_json::Value::as_u64),
        completion_tokens: value
            .pointer("/usage/completion_tokens")
            .and_then(serde_json::Value::as_u64),
        ..OpenAiStreamEvent::default()
    };
    let Some(choice) = value
        .get("choices")
        .and_then(serde_json::Value::as_array)
        .and_then(|choices| choices.first())
    else {
        return Ok(OpenAiStreamLine::Event(event));
    };
    event.finish_reason = choice
        .get("finish_reason")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let message = choice.get("delta").or_else(|| choice.get("message"));
    if let Some(message) = message {
        event.role = message
            .get("role")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        event.content = message
            .get("content")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        event.reasoning = message
            .get("reasoning_content")
            .or_else(|| message.get("reasoning"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        event.tool_calls = message
            .get("tool_calls")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();
    }
    Ok(OpenAiStreamLine::Event(event))
}

pub fn merge_openai_tool_call_deltas(
    target: &mut Vec<serde_json::Value>,
    incoming: &[serde_json::Value],
) {
    for (fallback_index, delta) in incoming.iter().enumerate() {
        let index = delta
            .get("index")
            .and_then(serde_json::Value::as_u64)
            .and_then(|index| usize::try_from(index).ok())
            .unwrap_or(fallback_index);
        while target.len() <= index {
            target.push(serde_json::json!({
                "id": "",
                "type": "function",
                "function": {"name": "", "arguments": ""},
            }));
        }
        let current = &mut target[index];
        if let Some(id) = delta.get("id").and_then(serde_json::Value::as_str)
            && !id.is_empty()
        {
            current["id"] = serde_json::Value::String(id.to_string());
        }
        if let Some(kind) = delta.get("type").and_then(serde_json::Value::as_str)
            && !kind.is_empty()
        {
            current["type"] = serde_json::Value::String(kind.to_string());
        }
        let Some(function) = delta.get("function") else {
            continue;
        };
        for key in ["name", "arguments"] {
            let Some(fragment) = function.get(key).and_then(serde_json::Value::as_str) else {
                continue;
            };
            let existing = current
                .pointer(&format!("/function/{key}"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            current["function"][key] = serde_json::Value::String(format!("{existing}{fragment}"));
        }
    }
}

pub fn tool_result_message(
    backend: InferenceBackend,
    call: &serde_json::Value,
    tool_name: &str,
    content: String,
) -> serde_json::Value {
    match backend {
        InferenceBackend::Ollama => serde_json::json!({
            "role": "tool",
            "tool_name": tool_name,
            "content": content,
        }),
        InferenceBackend::OpenVino => {
            let id = call
                .get("id")
                .and_then(serde_json::Value::as_str)
                .filter(|id| !id.is_empty())
                .unwrap_or(tool_name);
            serde_json::json!({
                "role": "tool",
                "tool_call_id": id,
                "content": content,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_connections_have_independent_defaults() {
        let connections = BackendConnections::default();
        assert_eq!(connections.ollama.port, "11434");
        assert_eq!(connections.openvino.port, "8000");
    }

    #[test]
    fn urls_support_https_and_ipv6_for_every_backend() {
        let location = HostLocation::new("https://", "2001:db8::1", "8443");
        assert_eq!(
            api_url(InferenceBackend::OpenVino, &location, "/v3/models").unwrap(),
            "https://[2001:db8::1]:8443/v3/models"
        );
    }

    #[test]
    fn legacy_addresses_load_as_one_url_without_losing_endpoints() {
        let connections: BackendConnections = serde_json::from_value(serde_json::json!({
            "ollama": {"protocol": "https://", "ip": "::1", "port": "11434"},
            "openvino": {
                "protocol": "http", "ip": "server.example", "port": "8000",
                "models_endpoint": "/v1/models", "chat_endpoint": "/v1/chat/completions"
            }
        }))
        .unwrap();
        assert_eq!(connections.ollama.address(), "https://[::1]:11434");
        assert_eq!(connections.openvino.address(), "http://server.example:8000");
        assert_eq!(
            api_url(
                InferenceBackend::OpenVino,
                &connections.openvino,
                "/v3/models"
            )
            .unwrap(),
            "http://server.example:8000/v1/models"
        );
        assert_eq!(connections.openvino.chat_endpoint, "/v1/chat/completions");
    }

    #[test]
    fn plaintext_addresses_preserve_base_paths_and_custom_endpoints() {
        for backend in InferenceBackend::ALL {
            for address in ["https://[::1]:8443/proxy", "https://[::1]:8443/proxy/"] {
                let mut location: HostLocation = serde_json::from_value(serde_json::json!({
                    "address": address,
                    "models_endpoint": "/v1/models",
                    "chat_endpoint": "/v1/chat/completions"
                }))
                .unwrap();
                let expected_path = if backend == InferenceBackend::Ollama {
                    "/api/chat"
                } else {
                    "/v1/chat/completions"
                };
                assert_eq!(
                    base_url(backend, &location).unwrap().as_str(),
                    "https://[::1]:8443/proxy/"
                );
                assert_eq!(
                    api_url(backend, &location, backend.chat_path()).unwrap(),
                    format!("https://[::1]:8443/proxy{expected_path}")
                );
                location.set_address("https://server.example".into());
                assert_eq!(
                    base_url(backend, &location).unwrap().as_str(),
                    "https://server.example/"
                );
            }
        }
    }

    #[test]
    fn address_edits_round_trip_and_remain_readable_by_older_versions() {
        #[derive(Deserialize)]
        struct LegacyLocation {
            protocol: String,
            ip: String,
            port: String,
        }

        for (address, host, port) in [
            ("  https://server.example:8443  ", "server.example", "8443"),
            ("https://server.example", "server.example", "443"),
            ("http://[::1]:11434", "[::1]", "11434"),
        ] {
            let mut location = InferenceBackend::Ollama.default_location();
            location.set_address(address.into());
            let saved = serde_json::to_value(&location).unwrap();
            let restored: HostLocation = serde_json::from_value(saved.clone()).unwrap();
            assert_eq!(restored, location);
            assert_eq!(restored.address(), address);
            let legacy: LegacyLocation = serde_json::from_value(saved).unwrap();
            assert_eq!(legacy.ip, host);
            assert_eq!(legacy.port, port);
            let old_location = HostLocation::new(&legacy.protocol, &legacy.ip, &legacy.port);
            assert_eq!(
                base_url(InferenceBackend::Ollama, &old_location).unwrap(),
                base_url(InferenceBackend::Ollama, &location).unwrap()
            );
        }
    }

    #[test]
    fn invalid_or_cleared_address_does_not_fall_back_to_the_previous_server() {
        for address in [
            "",
            "http://",
            "server.example:8000",
            "ftp://server.example",
            "http://server.example:0",
            "http://server.example:99999",
            "http://server.example?key=value",
            "http://server.example#fragment",
        ] {
            let mut location = InferenceBackend::OpenVino.default_location();
            location.set_address(address.into());
            let restored: HostLocation =
                serde_json::from_value(serde_json::to_value(&location).unwrap()).unwrap();
            assert_eq!(restored.address(), address);
            assert!(
                base_url(InferenceBackend::OpenVino, &restored).is_err(),
                "{address}"
            );
            assert_eq!(restored.port, "8000");
        }
    }

    #[test]
    fn openvino_models_are_read_from_openai_list_shape() {
        let names = model_names(
            InferenceBackend::OpenVino,
            &serde_json::json!({"data": [{"id": "z"}, {"id": "a"}, {"id": "a"}]}),
        )
        .unwrap();
        assert_eq!(names, vec!["a", "z"]);
    }

    #[test]
    fn openvino_images_use_openai_multimodal_content() {
        let message = user_message(
            InferenceBackend::OpenVino,
            "describe".into(),
            &[EncodedImage {
                mime_type: "image/png".into(),
                data: "YWJj".into(),
            }],
        );
        assert_eq!(message["content"][0]["text"], "describe");
        assert_eq!(
            message["content"][1]["image_url"]["url"],
            "data:image/png;base64,YWJj"
        );
    }

    #[test]
    fn openvino_chat_requests_use_the_openai_compatible_shape() {
        let tools = serde_json::json!([{
            "type": "function",
            "function": {"name": "web_search", "parameters": {"type": "object"}}
        }]);
        let body = chat_request_body(
            InferenceBackend::OpenVino,
            "qwen3",
            &[serde_json::json!({"role": "user", "content": "hello"})],
            Some(&tools),
            &serde_json::json!("high"),
            0.2,
            0.85,
            25,
            8_192,
            512,
        );

        assert_eq!(body["top_p"], serde_json::json!(0.85_f32));
        assert_eq!(body["top_k"], 25);
        assert_eq!(body["model"], "qwen3");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_tokens"], 512);
        assert_eq!(body["tool_choice"], "auto");
        assert_eq!(body["tools"], tools);
        assert_eq!(body["chat_template_kwargs"]["enable_thinking"], true);
        assert_eq!(body["chat_template_kwargs"]["reasoning_effort"], "high");
        assert!(body.get("options").is_none());
    }

    #[test]
    fn sampling_controls_reach_direct_and_tool_chat_requests() {
        for backend in InferenceBackend::ALL {
            let thinking = serde_json::json!(false);
            let direct = direct_request_body(
                backend,
                "model",
                "hello".into(),
                String::new(),
                &[],
                &thinking,
                0.7,
                0.75,
                17,
                8192,
                512,
            );
            let chat = chat_request_body(
                backend,
                "model",
                &[],
                Some(&serde_json::json!([])),
                &thinking,
                0.7,
                0.75,
                17,
                8192,
                512,
            );
            for body in [direct, chat] {
                let sampling = if backend == InferenceBackend::Ollama {
                    &body["options"]
                } else {
                    &body
                };
                assert_eq!(sampling["top_p"], 0.75);
                assert_eq!(sampling["top_k"], 17);
            }
        }
    }

    #[test]
    fn openvino_backend_has_a_stable_settings_name() {
        assert_eq!(
            serde_json::to_value(InferenceBackend::OpenVino).unwrap(),
            serde_json::json!("openvino")
        );
        assert_eq!(
            serde_json::from_value::<InferenceBackend>(serde_json::json!("open-vino")).unwrap(),
            InferenceBackend::OpenVino
        );
    }

    #[test]
    fn openai_stream_exposes_content_reasoning_and_finish_reason() {
        let line = r#"data: {"choices":[{"delta":{"role":"assistant","content":"Hi","reasoning_content":"Work"},"finish_reason":"length"}],"usage":{"completion_tokens":12}}"#;
        let OpenAiStreamLine::Event(event) = decode_openai_stream_line(line).unwrap() else {
            panic!("expected event");
        };
        assert_eq!(event.content, "Hi");
        assert_eq!(event.reasoning, "Work");
        assert_eq!(event.finish_reason.as_deref(), Some("length"));
        assert_eq!(event.completion_tokens, Some(12));
        assert_eq!(
            decode_openai_stream_line("data: [DONE]").unwrap(),
            OpenAiStreamLine::Done
        );
    }

    #[test]
    fn fragmented_openai_tool_calls_are_reassembled() {
        let mut calls = Vec::new();
        merge_openai_tool_call_deltas(
            &mut calls,
            &[serde_json::json!({
                "index": 0,
                "id": "call-1",
                "type": "function",
                "function": {"name": "web_search", "arguments": "{\"query\":"}
            })],
        );
        merge_openai_tool_call_deltas(
            &mut calls,
            &[serde_json::json!({
                "index": 0,
                "function": {"arguments": "\"rust\"}"}
            })],
        );
        assert_eq!(calls[0]["id"], "call-1");
        assert_eq!(calls[0]["function"]["name"], "web_search");
        assert_eq!(calls[0]["function"]["arguments"], r#"{"query":"rust"}"#);
    }
}
