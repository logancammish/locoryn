use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering as AtomicOrdering},
};
use std::thread;

// Some restricted CI/sandbox environments allow only one loopback listener
// to be created at a time.
static LOOPBACK_TEST_LOCK: Mutex<()> = Mutex::new(());

fn progress_sender() -> tokio::sync::watch::Sender<ToolLoopProgress> {
    tokio::sync::watch::channel(ToolLoopProgress::default()).0
}

fn read_http_request_bytes(stream: &mut std::net::TcpStream) -> Vec<u8> {
    let mut request = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let read = stream.read(&mut chunk).unwrap();
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        let Some(header_end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);
        if request.len() >= header_end + 4 + content_length {
            break;
        }
    }
    request
}

fn read_http_request(stream: &mut std::net::TcpStream) {
    let _ = read_http_request_bytes(stream);
}

struct CountingProvider(AtomicUsize);

#[async_trait]
impl WebSearchProvider for CountingProvider {
    async fn search(
        &self,
        _query: &str,
        _limit: usize,
        _freshness: WebSearchFreshness,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        self.0.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(Vec::new())
    }

    async fn fetch_page(&self, _url: &str) -> Result<WebPageContent, WebSearchError> {
        self.0.fetch_add(1, AtomicOrdering::Relaxed);
        Err(WebSearchError::InvalidUrl)
    }
}

#[test]
fn web_search_is_disabled_by_default_and_old_settings_load() {
    let settings: WebSearchSettings = serde_json::from_str("{}").unwrap();
    assert!(!settings.enabled);
    assert!(!settings.allow_multiple_searches);
    assert_eq!(settings.result_limit, DEFAULT_RESULT_LIMIT);
    assert_eq!(settings.maximum_searches, DEFAULT_MAX_SEARCHES_PER_MESSAGE);
    assert_eq!(settings.tool_iteration_limit, DEFAULT_TOOL_ITERATION_LIMIT);
}

#[test]
fn disabled_search_never_calls_the_provider() {
    let provider = CountingProvider(AtomicUsize::new(0));
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert!(matches!(
        runtime.block_on(guarded_search(
            false,
            Some(&provider),
            "query",
            5,
            WebSearchFreshness::Any,
        )),
        Err(WebSearchError::Disabled)
    ));
    assert!(matches!(
        runtime.block_on(guarded_fetch(false, Some(&provider), "https://example.com")),
        Err(WebSearchError::Disabled)
    ));
    assert_eq!(provider.0.load(AtomicOrdering::Relaxed), 0);
}

#[test]
fn settings_round_trip() {
    let settings = WebSearchSettings {
        enabled: true,
        allow_multiple_searches: true,
        api_key: Some("secret".into()),
        tavily_api_key: Some("tvly-secret".into()),
        exa_api_key: Some("exa-secret".into()),
        maximum_searches: 12,
        maximum_page_fetches: 9,
        minimum_successful_searches: 5,
        minimum_independent_pages: 4,
        tool_iteration_limit: 30,
        custom_research_instructions: "Prefer primary sources.".into(),
        ..WebSearchSettings::default()
    };
    let encoded = serde_json::to_string(&settings).unwrap();
    assert_eq!(
        serde_json::from_str::<WebSearchSettings>(&encoded).unwrap(),
        settings
    );
}

#[test]
fn provider_keys_are_saved_and_selected_independently() {
    let mut settings: WebSearchSettings =
        serde_json::from_str(r#"{"api_key":"existing-brave-key"}"#).unwrap();
    assert_eq!(settings.selected_api_key(), Some("existing-brave-key"));

    settings.provider = WebSearchProviderKind::Tavily;
    assert_eq!(settings.selected_api_key(), None);
    settings.set_selected_api_key(Some("  tvly-key  ".into()));
    assert_eq!(settings.selected_api_key(), Some("tvly-key"));

    settings.provider = WebSearchProviderKind::Exa;
    settings.set_selected_api_key(Some("exa-key".into()));
    assert_eq!(settings.selected_api_key(), Some("exa-key"));

    settings.provider = WebSearchProviderKind::Brave;
    assert_eq!(settings.selected_api_key(), Some("existing-brave-key"));
    assert_eq!(
        WebSearchProviderKind::ALL,
        [
            WebSearchProviderKind::Brave,
            WebSearchProviderKind::Tavily,
            WebSearchProviderKind::Exa,
        ]
    );
}

#[test]
fn configurable_research_limits_are_normalized_together() {
    let settings = WebSearchSettings {
        maximum_searches: 2,
        maximum_page_fetches: 1,
        minimum_successful_searches: 99,
        minimum_independent_pages: 99,
        tool_iteration_limit: usize::MAX,
        request_timeout_seconds: 1,
        custom_research_instructions: format!(
            "  {}  ",
            "x".repeat(MAX_CUSTOM_RESEARCH_INSTRUCTIONS_CHARS + 50)
        ),
        ..WebSearchSettings::default()
    }
    .normalized();

    assert_eq!(settings.minimum_successful_searches, 2);
    assert_eq!(settings.minimum_independent_pages, 1);
    assert_eq!(
        settings.tool_iteration_limit,
        MAX_CONFIGURABLE_TOOL_ITERATIONS
    );
    assert_eq!(settings.request_timeout_seconds, 3);
    assert_eq!(
        settings.custom_research_instructions.chars().count(),
        MAX_CUSTOM_RESEARCH_INSTRUCTIONS_CHARS
    );
}

#[test]
fn blocks_invalid_and_private_urls() {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    assert!(matches!(
        runtime.block_on(validate_public_url(&Url::parse("file:///tmp/a").unwrap())),
        Err(WebSearchError::UnsupportedScheme)
    ));
    assert!(matches!(
        runtime.block_on(validate_public_url(
            &Url::parse("http://127.0.0.1/private").unwrap()
        )),
        Err(WebSearchError::UnsafeAddress)
    ));
    assert!(matches!(
        runtime.block_on(validate_public_url(
            &Url::parse("http://[::1]/private").unwrap()
        )),
        Err(WebSearchError::UnsafeAddress)
    ));
}

#[test]
fn api_keys_are_redacted_from_diagnostics() {
    let error = WebSearchError::ProviderUnavailable("bad key secret-123".into());
    assert_eq!(
        error.diagnostic(Some("secret-123")),
        "provider unavailable: bad key <redacted>"
    );
    assert!(
        error
            .detailed_user_message(Some("secret-123"))
            .contains("bad key <redacted>")
    );
}

#[test]
fn inference_errors_distinguish_tool_support_from_context_overflow() {
    assert!(is_tools_unsupported_error(
        "Tool parser is not configured for this model"
    ));
    assert!(!is_tools_unsupported_error(
        "A tool returned an ordinary provider error"
    ));
    assert!(is_context_length_error(
        "LLMExecutor failed: Input length exceeds the maximum allowed length"
    ));
    assert!(!is_context_length_error(
        "temporary inference connection error"
    ));
    assert!(explicitly_requests_web_search(
        "Using web search, find the latest release"
    ));
    assert!(explicitly_requests_web_search(
        "Please search the web for this"
    ));
    assert!(!explicitly_requests_web_search(
        "Explain what a binary search is"
    ));
}

#[test]
fn streamed_chat_chunks_accumulate_visible_progress_and_tool_calls() {
    let (progress_sender, progress_receiver) =
        tokio::sync::watch::channel(ToolLoopProgress::default());
    let mut role = String::new();
    let mut content = String::new();
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let progress = StreamProgressContext {
        previous_thinking: "earlier",
        previous_answer: "kept draft",
        sender: &progress_sender,
    };

    apply_ollama_chat_stream_line(
            r#"{"message":{"role":"assistant","thinking":"checking ","content":"","tool_calls":[{"function":{"name":"web_search","arguments":{"query":"first"}}}]},"done":false}"#,
            &mut role,
            &mut content,
            &mut thinking,
            &mut tool_calls,
            &progress,
        )
        .unwrap();
    apply_ollama_chat_stream_line(
            r#"{"message":{"role":"assistant","thinking":"sources","content":"partial answer","tool_calls":[{"function":{"name":"fetch_webpage","arguments":{"url":"https://example.com"}}}]},"done":true}"#,
            &mut role,
            &mut content,
            &mut thinking,
            &mut tool_calls,
            &progress,
        )
        .unwrap();

    assert_eq!(thinking, "checking sources");
    assert_eq!(content, "partial answer");
    assert_eq!(tool_calls.len(), 2);
    assert_eq!(
        progress_receiver.borrow().answer,
        "kept draft\n\npartial answer"
    );
    assert_eq!(
        progress_receiver.borrow().thinking,
        "earlier\n\nchecking sources"
    );
}

#[test]
fn parses_successful_mocked_search_and_provider_failure() {
    let body: BraveResponse = serde_json::from_str(
            r#"{"web":{"results":[{"title":"Example","url":"https://example.com/page","description":"A result"}]}}"#,
        )
        .unwrap();
    let results = parse_brave_results(body, 3).unwrap();
    assert_eq!(results[0].title, "Example");
    assert!(matches!(
        map_status(StatusCode::SERVICE_UNAVAILABLE),
        Err(WebSearchError::ProviderUnavailable(_))
    ));
}

#[test]
fn parses_experimental_provider_results() {
    let tavily: TavilyResponse = serde_json::from_str(
            r#"{"results":[{"title":"Tavily result","url":"https://example.com/tavily","content":"Relevant Tavily content"}]}"#,
        )
        .unwrap();
    assert_eq!(
        parse_tavily_results(tavily, 5).unwrap()[0],
        WebSearchResult {
            title: "Tavily result".into(),
            url: "https://example.com/tavily".into(),
            snippet: "Relevant Tavily content".into(),
        }
    );

    let exa: ExaResponse = serde_json::from_str(
            r#"{"results":[{"title":"Exa result","url":"https://example.com/exa","highlights":["First excerpt","Second excerpt"]}]}"#,
        )
        .unwrap();
    assert_eq!(
        parse_exa_results(exa, 5).unwrap()[0].snippet,
        "First excerpt [...] Second excerpt"
    );
}

#[test]
fn experimental_provider_requests_map_limits_and_freshness() {
    let tavily = tavily_request_body("release notes", 99, WebSearchFreshness::Month);
    assert_eq!(tavily["max_results"], MAX_RESULT_LIMIT);
    assert_eq!(tavily["time_range"], "month");
    assert_eq!(tavily["include_answer"], false);

    let now = chrono::DateTime::parse_from_rfc3339("2026-07-27T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let exa = exa_request_body("release notes", 3, WebSearchFreshness::Week, now);
    assert_eq!(exa["numResults"], 3);
    assert_eq!(exa["startPublishedDate"], "2026-07-20T00:00:00.000Z");
    assert_eq!(exa["contents"]["highlights"], true);
}

#[test]
fn source_numbers_are_stable_and_deduplicated() {
    let mut sources = Vec::new();
    assert_eq!(
        add_source(&mut sources, "First".into(), "https://example.com".into()),
        1
    );
    assert_eq!(
        add_source(&mut sources, "Updated".into(), "https://example.com".into()),
        1
    );
    assert_eq!(sources.len(), 1);
}

#[test]
fn search_state_updates_do_not_wait_for_the_renderer() {
    let (sender, receiver) = crossbeam_channel::unbounded();
    let searching = WebSearchState::Searching {
        query: "rust iced".into(),
        websites: Vec::new(),
    };

    set_state(&sender, searching.clone());
    assert_eq!(receiver.try_recv().unwrap(), searching);

    // Closing the window disconnects the receiver. A worker finishing
    // afterwards must still be able to exit without blocking or panicking.
    drop(receiver);
    set_state(&sender, WebSearchState::Completed);
}

#[test]
fn strips_scripts_and_markup_from_webpages() {
    let html = r#"<html><head><title>Example &amp; Test</title><script>steal()</script></head><body><h1>Hello</h1><style>body{display:none}</style><p>World</p></body></html>"#;
    assert_eq!(html_title(html).as_deref(), Some("Example & Test"));
    let text = html_to_text(html);
    assert!(text.contains("Hello"));
    assert!(text.contains("World"));
    assert!(!text.contains("steal"));
    assert!(!text.contains("display:none"));
}

#[test]
fn tool_limits_are_bounded() {
    let single_settings = WebSearchSettings::default();
    let mut single_search_budget = ToolBudget::new(&single_settings);
    for _ in 0..(DEFAULT_SEARCHES_PER_MESSAGE
        + DEFAULT_PAGES_PER_MESSAGE
        + MAX_STALLED_RESEARCH_REMINDERS
        + 1)
    {
        assert!(single_search_budget.take_iteration());
    }
    assert!(!single_search_budget.take_iteration());
    assert!(single_search_budget.take_search());
    assert!(!single_search_budget.take_search());
    assert!(single_search_budget.take_code_check());
    assert!(!single_search_budget.take_code_check());
    for _ in 0..CONVERSATION_SEARCHES_PER_MESSAGE {
        assert!(single_search_budget.take_conversation_search());
    }
    assert!(!single_search_budget.take_conversation_search());

    let multiple_settings = WebSearchSettings {
        allow_multiple_searches: true,
        ..WebSearchSettings::default()
    };
    let mut multiple_search_budget = ToolBudget::new(&multiple_settings);
    for _ in 0..MAX_TOOL_ITERATIONS {
        assert!(multiple_search_budget.take_iteration());
    }
    assert!(!multiple_search_budget.take_iteration());
    for _ in 0..DEFAULT_MAX_SEARCHES_PER_MESSAGE {
        assert!(multiple_search_budget.take_search());
    }
    assert!(!multiple_search_budget.take_search());
    for _ in 0..DEFAULT_MAX_PAGES_PER_MESSAGE {
        assert!(multiple_search_budget.take_page());
    }
    assert!(!multiple_search_budget.take_page());
}

#[test]
fn tool_guidance_matches_the_repeated_search_setting() {
    let single_settings = WebSearchSettings::default();
    let research_settings = WebSearchSettings {
        allow_multiple_searches: true,
        custom_research_instructions: "Prefer standards documents.".into(),
        ..WebSearchSettings::default()
    };
    let tool_settings = crate::tools::ToolSettings::default();
    let single_guidance = tool_loop_guidance(&single_settings, &tool_settings, false, "2026-07-26");
    let research_guidance =
        tool_loop_guidance(&research_settings, &tool_settings, false, "2026-07-26");
    assert!(single_guidance.contains("at most once"));
    assert!(single_guidance.contains("2026-07-26"));
    assert!(research_guidance.contains("Run 3 to 6"));
    assert!(research_guidance.contains("at least 2 independent"));
    assert!(research_guidance.contains("Prefer standards documents."));
    assert!(single_guidance.contains("1-5 distinctive"));
    assert!(single_guidance.contains("not a question or instruction"));

    let single_search_tools = tool_definitions(&single_settings);
    let repeated_search_tools = tool_definitions(&research_settings);
    assert!(
        single_search_tools[0]["function"]["description"]
            .as_str()
            .unwrap()
            .contains("once")
    );
    assert!(
        repeated_search_tools[0]["function"]["description"]
            .as_str()
            .unwrap()
            .contains("3 to 6")
    );
    assert_eq!(
        repeated_search_tools[0]["function"]["parameters"]["properties"]["result_count"]["maximum"],
        5
    );
    assert_eq!(
        repeated_search_tools[0]["function"]["parameters"]["properties"]["freshness"]["enum"],
        serde_json::json!(["any", "day", "week", "month", "year"])
    );

    let search_only_tools = tool_definitions(&WebSearchSettings {
        allow_multiple_searches: true,
        maximum_page_fetches: 0,
        minimum_independent_pages: 0,
        ..WebSearchSettings::default()
    });
    assert_eq!(search_only_tools.as_array().unwrap().len(), 3);
}

#[test]
fn exhausted_tools_are_not_offered_to_the_model_again() {
    let settings = WebSearchSettings {
        allow_multiple_searches: true,
        maximum_searches: 1,
        maximum_page_fetches: 1,
        minimum_successful_searches: 1,
        minimum_independent_pages: 0,
        ..WebSearchSettings::default()
    };
    let mut budget = ToolBudget::new(&settings);
    let tool_settings = crate::tools::ToolSettings {
        enabled: true,
        ..crate::tools::ToolSettings::default()
    };
    let names = |definitions: serde_json::Value| {
        definitions
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|definition| definition["function"]["name"].as_str().map(str::to_string))
            .collect::<Vec<_>>()
    };

    assert_eq!(
        names(available_tool_definitions(
            &settings,
            &tool_settings,
            false,
            &budget
        )),
        vec![
            "web_search",
            "fetch_webpage",
            "search_locoryn_conversations"
        ]
    );
    assert!(budget.take_search());
    assert_eq!(
        names(available_tool_definitions(
            &settings,
            &tool_settings,
            false,
            &budget
        )),
        vec!["fetch_webpage", "search_locoryn_conversations"]
    );
    assert!(budget.take_page());
    assert_eq!(
        names(available_tool_definitions(
            &settings,
            &tool_settings,
            false,
            &budget
        )),
        vec!["search_locoryn_conversations"]
    );
}

#[test]
fn code_check_tool_requires_both_consent_switches() {
    let settings = WebSearchSettings::default();
    let budget = ToolBudget::new(&settings);
    let names = |definitions: serde_json::Value| {
        definitions
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|definition| definition["function"]["name"].as_str().map(str::to_string))
            .collect::<Vec<_>>()
    };
    let tools = crate::tools::ToolSettings {
        web_search: false,
        fetch_webpage: false,
        conversation_search: false,
        code_checking: true,
        ..crate::tools::ToolSettings::default()
    };

    assert!(
        names(available_tool_definitions(
            &settings, &tools, false, &budget
        ))
        .is_empty()
    );
    assert_eq!(
        names(available_tool_definitions(&settings, &tools, true, &budget)),
        vec!["check_code".to_string()]
    );

    let disabled_tools = crate::tools::ToolSettings {
        enabled: false,
        ..tools
    };
    assert!(
        names(available_tool_definitions(
            &settings,
            &disabled_tools,
            true,
            &budget
        ))
        .is_empty()
    );
}

#[test]
fn search_calls_can_choose_bounded_breadth_and_freshness() {
    let arguments = serde_json::json!({
        "query": "current release notes",
        "result_count": 3,
        "freshness": "week",
    });
    assert_eq!(requested_result_count(&arguments, 5).unwrap(), 3);
    assert_eq!(
        WebSearchFreshness::from_tool_value(arguments.get("freshness")).unwrap(),
        WebSearchFreshness::Week
    );
    assert_eq!(WebSearchFreshness::Week.brave_value(), Some("pw"));
    assert_eq!(WebSearchFreshness::Week.tavily_value(), Some("week"));
    assert_eq!(WebSearchFreshness::Week.exa_lookback_days(), Some(7));

    assert!(matches!(
        requested_result_count(&serde_json::json!({"result_count": 6}), 5),
        Err(WebSearchError::InvalidToolCall)
    ));
    assert!(matches!(
        WebSearchFreshness::from_tool_value(Some(&serde_json::json!("decade"))),
        Err(WebSearchError::InvalidToolCall)
    ));
    assert!(matches!(
        WebSearchFreshness::from_tool_value(Some(&serde_json::json!(7))),
        Err(WebSearchError::InvalidToolCall)
    ));
}

#[test]
fn malformed_tool_calls_are_returned_as_recoverable_feedback() {
    let message = invalid_tool_message(
        InferenceBackend::Ollama,
        &serde_json::json!({}),
        "search_locoryn_conversations",
        "search_locoryn_conversations requires a non-empty string query.",
    );

    assert_eq!(message["role"], "tool");
    assert_eq!(message["tool_name"], "search_locoryn_conversations");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(message["content"].as_str().unwrap()).unwrap()["error"],
        "invalid tool call"
    );
}

#[test]
fn conversation_search_can_retry_with_a_broader_query_and_then_answer() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let responses = [
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "search_locoryn_conversations",
                        "arguments": {"query": "lighthouse deployment deadline"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "search_locoryn_conversations",
                        "arguments": {"query": "lighthouse deployment"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "The saved chat says the lighthouse deployment is next week."
            }
        })
        .to_string(),
    ];
    let server = thread::spawn(move || {
        for body in responses {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });

    let dir = std::env::temp_dir().join("locoryn-test-tool-loop-conversation-retry");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let chats = serde_json::json!([{
        "id": "matching-chat",
        "profile": "profile-a",
        "title": "Lighthouse plan",
        "updated_at": "2026-01-01T00:00:00Z",
        "messages": [
            {"role": "user", "text": "When is the lighthouse deployment?"},
            {"role": "bot", "text": "It is planned for next week."}
        ]
    }]);
    std::fs::write(dir.join("chats.json"), chats.to_string()).unwrap();

    let (progress_sender, _) = tokio::sync::watch::channel(ToolLoopProgress::default());
    let request = ToolLoopRequest {
        backend: InferenceBackend::Ollama,
        chat_url: format!("http://{address}/api/chat"),
        model: "test-model".into(),
        prompt: "What did we decide about the lighthouse deployment?".into(),
        user_prompt: "What did we decide about the lighthouse deployment?".into(),
        system_prompt: "test system prompt".into(),
        temperature: 0.0,
        context_tokens: 4_096,
        max_response_tokens: 512,
        images: Vec::new(),
        thinking: serde_json::Value::Bool(false),
        settings: WebSearchSettings::default(),
        tool_settings: crate::tools::ToolSettings {
            web_search: false,
            fetch_webpage: false,
            ..crate::tools::ToolSettings::default()
        },
        code_checking_enabled: false,
        provider: None,
        state_sender: crossbeam_channel::unbounded().0,
        progress_sender,
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: Some(dir.clone()),
        conversation_profile_id: "profile-a".into(),
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    server.join().unwrap();

    assert_eq!(
        result.answer,
        "The saved chat says the lighthouse deployment is next week."
    );
    assert!(result.sources.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn page_excerpt_budget_scales_with_context_but_stays_bounded() {
    assert_eq!(page_text_limit(4_096, 6), 2_000);
    assert_eq!(page_text_limit(1_000_000, 6), MAX_PAGE_TEXT_CHARS);
}

#[test]
fn research_checkpoint_only_activates_after_web_research_starts() {
    let settings = WebSearchSettings {
        allow_multiple_searches: true,
        ..WebSearchSettings::default()
    };
    let mut budget = ToolBudget::new(&settings);
    assert!(research_checkpoint(&budget, 0, &[], &[], &settings).is_none());

    assert!(budget.take_search());
    let checkpoint = research_checkpoint(&budget, 0, &[], &[], &settings).unwrap();
    assert!(checkpoint.contains("3 more targeted"));
}

#[test]
fn repeated_search_queries_are_compared_case_and_whitespace_insensitively() {
    assert_eq!(
        normalize_search_query("  Rust   Iced\nGUI "),
        normalize_search_query("rust iced gui")
    );
    assert_ne!(
        normalize_search_query("rust iced gui"),
        normalize_search_query("rust iced tutorial")
    );
}

#[test]
fn web_tool_loop_user_message_keeps_all_images() {
    let message = user_message(
        InferenceBackend::Ollama,
        "Compare these images".into(),
        &[
            EncodedImage {
                mime_type: "image/png".into(),
                data: "first-image".into(),
            },
            EncodedImage {
                mime_type: "image/png".into(),
                data: "second-image".into(),
            },
        ],
    );

    assert_eq!(message["role"], "user");
    assert_eq!(message["content"], "Compare these images");
    assert_eq!(
        message["images"],
        serde_json::json!(["first-image", "second-image"])
    );
}

#[test]
fn ollama_inference_does_not_use_the_external_web_timeout() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_http_request(&mut stream);
        thread::sleep(Duration::from_millis(50));
        let body = concat!(
            "{\"message\":{\"role\":\"assistant\",\"thinking\":\"checked \",\"content\":\"\"},\"done\":false}\n",
            "{\"message\":{\"role\":\"assistant\",\"thinking\":\"the evidence\",\"content\":\"do\"},\"done\":false}\n",
            "{\"message\":{\"role\":\"assistant\",\"content\":\"ne\"},\"done\":true,\"eval_count\":120,\"eval_duration\":2000000000}\n"
        );
        write!(
                stream,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
    });

    let (progress_sender, mut progress_receiver) =
        tokio::sync::watch::channel(ToolLoopProgress::default());
    let request = ToolLoopRequest {
        backend: InferenceBackend::Ollama,
        chat_url: format!("http://{address}/api/chat"),
        model: "test-model".into(),
        prompt: "test prompt".into(),
        user_prompt: "test prompt".into(),
        system_prompt: "test system prompt".into(),
        temperature: 0.0,
        context_tokens: 4_096,
        max_response_tokens: 512,
        images: Vec::new(),
        thinking: serde_json::Value::Bool(false),
        settings: WebSearchSettings {
            enabled: true,
            request_timeout_seconds: 0,
            ..WebSearchSettings::default()
        },
        provider: Some(Arc::new(CountingProvider(AtomicUsize::new(0)))),
        state_sender: crossbeam_channel::unbounded().0,
        progress_sender,
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: None,
        conversation_profile_id: crate::app::LEGACY_PROFILE_ID.to_string(),
        tool_settings: crate::tools::ToolSettings::default(),
        code_checking_enabled: false,
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    assert_eq!(result.answer, "done");
    assert_eq!(result.thinking, "checked the evidence");
    assert_eq!(result.eval_count, Some(120));
    assert_eq!(result.eval_duration, Some(2_000_000_000));
    assert_eq!(progress_receiver.borrow_and_update().answer, "done");
    server.join().unwrap();
}

#[test]
fn openvino_inference_reads_openai_compatible_sse() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        read_http_request(&mut stream);
        let generated = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"reasoning_content\":\"checked the backend\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"OpenVINO \"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"ready\"},\"finish_reason\":\"stop\"}]}\n\n"
        );
        let completed = concat!(
            "data: {\"choices\":[],\"usage\":{\"completion_tokens\":2}}\n\n",
            "data: [DONE]\n\n"
        );
        let body_len = generated.len() + completed.len();
        write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {body_len}\r\nconnection: close\r\n\r\n{generated}"
        )
        .unwrap();
        stream.flush().unwrap();
        // Keep the usage terminator in a later network write so the client-side
        // OpenVINO generation timer observes a real interval.
        thread::sleep(Duration::from_millis(20));
        write!(stream, "{completed}").unwrap();
    });

    let (progress_sender, mut progress_receiver) =
        tokio::sync::watch::channel(ToolLoopProgress::default());
    let request = ToolLoopRequest {
        backend: InferenceBackend::OpenVino,
        chat_url: format!("http://{address}/v3/chat/completions"),
        model: "qwen3".into(),
        prompt: "test prompt".into(),
        user_prompt: "test prompt".into(),
        system_prompt: "test system prompt".into(),
        temperature: 0.0,
        context_tokens: 4_096,
        max_response_tokens: 512,
        images: Vec::new(),
        thinking: serde_json::Value::Bool(true),
        settings: WebSearchSettings {
            enabled: true,
            ..WebSearchSettings::default()
        },
        provider: Some(Arc::new(CountingProvider(AtomicUsize::new(0)))),
        state_sender: crossbeam_channel::unbounded().0,
        progress_sender,
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: None,
        conversation_profile_id: crate::app::LEGACY_PROFILE_ID.to_string(),
        tool_settings: crate::tools::ToolSettings {
            web_search: false,
            fetch_webpage: false,
            ..crate::tools::ToolSettings::default()
        },
        code_checking_enabled: false,
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    assert_eq!(result.answer, "OpenVINO ready");
    assert_eq!(result.thinking, "checked the backend");
    assert_eq!(result.eval_count, Some(2));
    assert!(result.eval_duration.is_some_and(|duration| duration > 0));
    assert_eq!(
        progress_receiver.borrow_and_update().answer,
        "OpenVINO ready"
    );
    server.join().unwrap();
}

#[test]
fn transient_ollama_rate_limits_are_retried() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        read_http_request(&mut first);
        let body = r#"{"error":"too many requests"}"#;
        write!(
                first,
                "HTTP/1.1 429 Too Many Requests\r\nretry-after: 0\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();

        let (mut second, _) = listener.accept().unwrap();
        read_http_request(&mut second);
        let body = r#"{"message":{"role":"assistant","content":"done"}}"#;
        write!(
                second,
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
    });

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = Client::new();
    let cancel = AtomicBool::new(false);
    let response = runtime
        .block_on(send_inference_request_with_retry(
            &client,
            &format!("http://{address}/api/chat"),
            &serde_json::json!({"model": "test-model"}),
            &cancel,
        ))
        .unwrap()
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    server.join().unwrap();
}

struct QueryRecordingProvider {
    queries: Mutex<Vec<(String, usize, WebSearchFreshness)>>,
    pages: Mutex<Vec<String>>,
}

#[test]
fn openvino_prompt_overflow_retries_with_compact_web_synthesis() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let (mut native_request, _) = listener.accept().unwrap();
        let native_payload = read_http_request_bytes(&mut native_request);
        assert!(String::from_utf8_lossy(&native_payload).contains("\"tools\""));
        let error = serde_json::json!({
            "error": {
                "message": "Mediapipe execution failed: Input length exceeds the maximum allowed length"
            }
        })
        .to_string();
        write!(
            native_request,
            "HTTP/1.1 400 Bad Request\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{error}",
            error.len()
        )
        .unwrap();

        let (mut compact_request, _) = listener.accept().unwrap();
        let compact_payload = read_http_request_bytes(&mut compact_request);
        request_sender.send(compact_payload).unwrap();
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"Verified result [1].\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":180,\"completion_tokens\":4}}\n\n",
            "data: [DONE]\n\n"
        );
        write!(
            compact_request,
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });

    let provider = Arc::new(QueryRecordingProvider {
        queries: Mutex::new(Vec::new()),
        pages: Mutex::new(Vec::new()),
    });
    let request = ToolLoopRequest {
        backend: InferenceBackend::OpenVino,
        chat_url: format!("http://{address}/v3/chat/completions"),
        model: "qwen3-npu".into(),
        prompt: format!("old conversation {}", "very long context ".repeat(2_000)),
        user_prompt: "find the current stable release".into(),
        system_prompt: "Be helpful.".into(),
        temperature: 0.0,
        context_tokens: 131_072,
        max_response_tokens: 32_768,
        images: Vec::new(),
        thinking: serde_json::Value::Bool(true),
        settings: WebSearchSettings {
            enabled: true,
            ..WebSearchSettings::default()
        },
        provider: Some(provider.clone()),
        state_sender: crossbeam_channel::unbounded().0,
        progress_sender: progress_sender(),
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: None,
        conversation_profile_id: crate::app::LEGACY_PROFILE_ID.to_string(),
        tool_settings: crate::tools::ToolSettings::default(),
        code_checking_enabled: false,
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    server.join().unwrap();
    let compact_payload = request_receiver.recv().unwrap();
    let compact_payload = String::from_utf8_lossy(&compact_payload);

    assert_eq!(result.answer, "Verified result [1].");
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.generation_details.prompt_tokens, Some(180));
    assert!(!compact_payload.contains("\"tools\""));
    assert!(!compact_payload.contains("very long context"));
    assert!(compact_payload.len() < 8_000);
    assert_eq!(
        provider.queries.lock().unwrap()[0].0,
        "find the current stable release"
    );
}

#[async_trait]
impl WebSearchProvider for QueryRecordingProvider {
    async fn search(
        &self,
        query: &str,
        limit: usize,
        freshness: WebSearchFreshness,
    ) -> Result<Vec<WebSearchResult>, WebSearchError> {
        self.queries
            .lock()
            .unwrap()
            .push((query.to_string(), limit, freshness));
        Ok(vec![WebSearchResult {
            title: format!("{query} result"),
            url: format!("https://{query}.example/article"),
            snippet: format!("Result for {query}"),
        }])
    }

    async fn fetch_page(&self, url: &str) -> Result<WebPageContent, WebSearchError> {
        self.pages.lock().unwrap().push(url.to_string());
        Ok(WebPageContent {
            url: url.to_string(),
            title: Some(format!("Page for {url}")),
            text: format!("Evidence from {url}"),
        })
    }
}

#[test]
fn tool_round_limit_forces_final_synthesis_without_losing_progress() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let responses = [
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "Initial research notes",
                "tool_calls": [{
                    "function": {
                        "name": "web_search",
                        "arguments": {"query": "first"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "web_search",
                        "arguments": {"query": "over budget"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "Final answer from the evidence [1]."
            }
        })
        .to_string(),
    ];
    let server = thread::spawn(move || {
        for body in responses {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
        }
    });

    let provider = Arc::new(QueryRecordingProvider {
        queries: Mutex::new(Vec::new()),
        pages: Mutex::new(Vec::new()),
    });
    let (progress_sender, mut progress_receiver) =
        tokio::sync::watch::channel(ToolLoopProgress::default());
    let request = ToolLoopRequest {
        backend: InferenceBackend::Ollama,
        chat_url: format!("http://{address}/api/chat"),
        model: "test-model".into(),
        prompt: "research this".into(),
        user_prompt: "research this".into(),
        system_prompt: "test system prompt".into(),
        temperature: 0.0,
        context_tokens: 4_096,
        max_response_tokens: 512,
        images: Vec::new(),
        thinking: serde_json::Value::Bool(false),
        settings: WebSearchSettings {
            enabled: true,
            allow_multiple_searches: true,
            maximum_searches: 1,
            maximum_page_fetches: 1,
            minimum_successful_searches: 1,
            minimum_independent_pages: 0,
            tool_iteration_limit: 2,
            ..WebSearchSettings::default()
        },
        provider: Some(provider.clone()),
        state_sender: crossbeam_channel::unbounded().0,
        progress_sender,
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: None,
        conversation_profile_id: crate::app::LEGACY_PROFILE_ID.to_string(),
        tool_settings: crate::tools::ToolSettings::default(),
        code_checking_enabled: false,
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    server.join().unwrap();

    assert_eq!(result.answer, "Final answer from the evidence [1].");
    assert_eq!(provider.queries.lock().unwrap().len(), 1);
    assert_eq!(
        progress_receiver.borrow_and_update().answer,
        "Final answer from the evidence [1]."
    );
}

#[test]
fn empty_limit_synthesis_gets_a_clean_no_tools_recovery() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let responses = [
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "thinking": "I should search.",
                "tool_calls": [{
                    "function": {
                        "name": "web_search",
                        "arguments": {"query": "first"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "thinking": "The first synthesis did not produce visible output."
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "Recovered answer from the collected evidence [1]."
            }
        })
        .to_string(),
    ];
    let server = thread::spawn(move || {
        for body in responses {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
        }
    });

    let provider = Arc::new(QueryRecordingProvider {
        queries: Mutex::new(Vec::new()),
        pages: Mutex::new(Vec::new()),
    });
    let (progress_sender, mut progress_receiver) =
        tokio::sync::watch::channel(ToolLoopProgress::default());
    let request = ToolLoopRequest {
        backend: InferenceBackend::Ollama,
        chat_url: format!("http://{address}/api/chat"),
        model: "test-model".into(),
        prompt: "research this".into(),
        user_prompt: "research this".into(),
        system_prompt: "test system prompt".into(),
        temperature: 0.0,
        context_tokens: 4_096,
        max_response_tokens: 512,
        images: Vec::new(),
        thinking: serde_json::Value::String("high".into()),
        settings: WebSearchSettings {
            enabled: true,
            allow_multiple_searches: true,
            maximum_searches: 1,
            maximum_page_fetches: 0,
            minimum_successful_searches: 1,
            minimum_independent_pages: 0,
            tool_iteration_limit: 2,
            ..WebSearchSettings::default()
        },
        provider: Some(provider.clone()),
        state_sender: crossbeam_channel::unbounded().0,
        progress_sender,
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: None,
        conversation_profile_id: crate::app::LEGACY_PROFILE_ID.to_string(),
        tool_settings: crate::tools::ToolSettings::default(),
        code_checking_enabled: false,
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    server.join().unwrap();

    assert_eq!(
        result.answer,
        "Recovered answer from the collected evidence [1]."
    );
    assert_eq!(provider.queries.lock().unwrap().len(), 1);
    assert_eq!(
        progress_receiver.borrow_and_update().answer,
        "Recovered answer from the collected evidence [1]."
    );
}

#[test]
fn follow_up_research_rejects_one_broad_search_and_cross_references_sources() {
    let _loopback_guard = LOOPBACK_TEST_LOCK.lock().unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let responses = [
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "web_search",
                        "arguments": {
                            "query": "first",
                            "result_count": 3,
                            "freshness": "week"
                        }
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "premature answer after one broad search"
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "web_search",
                        "arguments": {"query": "second"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "still premature after two searches"
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "web_search",
                        "arguments": {"query": "third"}
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "premature before reading sources"
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "fetch_webpage",
                        "arguments": {
                            "url": "https://first.example/article"
                        }
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "premature after reading one site"
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "fetch_webpage",
                        "arguments": {
                            "url": "https://second.example/article"
                        }
                    }
                }]
            }
        })
        .to_string(),
        serde_json::json!({
            "message": {
                "role": "assistant",
                "content": "cross-referenced answer"
            }
        })
        .to_string(),
    ];
    let server = thread::spawn(move || {
        for body in responses {
            let (mut stream, _) = listener.accept().unwrap();
            read_http_request(&mut stream);
            write!(
                    stream,
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
        }
    });

    let provider = Arc::new(QueryRecordingProvider {
        queries: Mutex::new(Vec::new()),
        pages: Mutex::new(Vec::new()),
    });
    let (state_sender, state_receiver) = crossbeam_channel::unbounded();
    let request = ToolLoopRequest {
        backend: InferenceBackend::Ollama,
        chat_url: format!("http://{address}/api/chat"),
        model: "test-model".into(),
        prompt: "research this current topic thoroughly".into(),
        user_prompt: "research this current topic thoroughly".into(),
        system_prompt: "test system prompt".into(),
        temperature: 0.0,
        context_tokens: 4_096,
        max_response_tokens: 512,
        images: Vec::new(),
        thinking: serde_json::Value::Bool(false),
        settings: WebSearchSettings {
            enabled: true,
            allow_multiple_searches: true,
            ..WebSearchSettings::default()
        },
        provider: Some(provider.clone()),
        state_sender,
        progress_sender: progress_sender(),
        cancel: Arc::new(AtomicBool::new(false)),
        chat_storage_dir: None,
        conversation_profile_id: crate::app::LEGACY_PROFILE_ID.to_string(),
        tool_settings: crate::tools::ToolSettings::default(),
        code_checking_enabled: false,
    };

    let runtime = tokio::runtime::Runtime::new().unwrap();
    let result = runtime.block_on(run_tool_loop(request)).unwrap();
    server.join().unwrap();

    assert_eq!(result.answer, "cross-referenced answer");
    assert_eq!(result.sources.len(), 3);
    assert_eq!(
        *provider.queries.lock().unwrap(),
        vec![
            ("first".to_string(), 3, WebSearchFreshness::Week),
            ("second".to_string(), 5, WebSearchFreshness::Any),
            ("third".to_string(), 5, WebSearchFreshness::Any),
        ]
    );
    assert_eq!(
        *provider.pages.lock().unwrap(),
        vec![
            "https://first.example/article".to_string(),
            "https://second.example/article".to_string(),
        ]
    );

    let states = state_receiver.try_iter().collect::<Vec<_>>();
    assert!(
        states
            .iter()
            .any(|state| { matches!(state, WebSearchState::Synthesizing { .. }) })
    );
    let result_states = states
        .into_iter()
        .filter_map(|state| match state {
            WebSearchState::Results { query, websites } => Some((query, websites)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(result_states.len(), 3);
    assert_eq!(result_states[0].0, "first");
    assert_eq!(result_states[0].1[0].url, "https://first.example/article");
    assert_eq!(result_states[1].0, "second");
    assert!(
        result_states[1]
            .1
            .iter()
            .any(|source| source.url == "https://second.example/article")
    );
    assert_eq!(result_states[2].0, "third");
    assert_eq!(result_states[2].1.len(), 3);
}

// --- Tests for the fixes ---

#[test]
fn normalized_host_strips_www_and_uses_registrable_domain() {
    assert_eq!(
        normalized_host("https://www.example.com/page").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        normalized_host("https://blog.example.com/post").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        normalized_host("https://docs.example.com/reference").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        normalized_host("https://sub.domain.example.com/page").as_deref(),
        Some("example.com")
    );
}

#[test]
fn normalized_host_handles_multi_part_tlds() {
    assert_eq!(
        normalized_host("https://www.example.co.uk/page").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(
        normalized_host("https://blog.example.co.uk/post").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(
        normalized_host("https://www.example.com.au/page").as_deref(),
        Some("example.com.au")
    );
    assert_eq!(
        normalized_host("https://www.example.co.jp/page").as_deref(),
        Some("example.co.jp")
    );
}

#[test]
fn normalized_host_handles_single_label() {
    assert_eq!(
        normalized_host("https://localhost/page").as_deref(),
        Some("localhost")
    );
}

#[test]
fn normalized_url_strips_tracking_params_and_fragments() {
    let a = normalized_url_for_dedup("https://example.com/page?utm_source=twitter&id=42");
    let b = normalized_url_for_dedup("https://example.com/page?utm_source=facebook&id=42");
    assert_eq!(a, b);
    let resolved = a.unwrap();
    assert!(resolved.contains("id=42"));
    assert!(!resolved.contains("utm_source"));
}

#[test]
fn normalized_url_strips_fragments() {
    let a = normalized_url_for_dedup("https://example.com/page#section1");
    let b = normalized_url_for_dedup("https://example.com/page#section2");
    assert_eq!(a, b);
}

#[test]
fn normalized_url_strips_gclid_and_fbclid() {
    let a = normalized_url_for_dedup("https://example.com/page?fbclid=abc&id=1");
    let b = normalized_url_for_dedup("https://example.com/page?gclid=xyz&id=1");
    assert_eq!(a, b);
    assert!(a.unwrap().contains("id=1"));
}

#[test]
fn source_dedup_uses_normalized_urls() {
    let mut sources: Vec<WebSource> = Vec::new();
    let idx1 = add_source(
        &mut sources,
        "Title".into(),
        "https://example.com/page?utm_source=a".into(),
    );
    let idx2 = add_source(
        &mut sources,
        "Title".into(),
        "https://example.com/page?utm_source=b".into(),
    );
    assert_eq!(idx1, idx2);
    assert_eq!(sources.len(), 1);
}

#[test]
fn source_dedup_still_allows_different_paths() {
    let mut sources: Vec<WebSource> = Vec::new();
    let idx1 = add_source(
        &mut sources,
        "Page A".into(),
        "https://example.com/page-a".into(),
    );
    let idx2 = add_source(
        &mut sources,
        "Page B".into(),
        "https://example.com/page-b".into(),
    );
    assert_ne!(idx1, idx2);
    assert_eq!(sources.len(), 2);
}

#[test]
fn html_to_text_strips_nav_footer_header_aside() {
    let html = "<html><body><nav>Navigation links</nav><main>Article content</main><footer>Copyright</footer></body></html>";
    let text = html_to_text(html);
    assert!(!text.contains("Navigation"));
    assert!(text.contains("Article content"));
    assert!(!text.contains("Copyright"));
}

#[test]
fn html_to_text_strips_hidden_elements() {
    let html = "<html><body><div>Visible</div><div hidden>Hidden content</div></body></html>";
    let text = html_to_text(html);
    assert!(text.contains("Visible"));
    assert!(!text.contains("Hidden content"));
}

#[test]
fn html_to_text_strips_display_none_elements() {
    let html =
        "<html><body><div>Visible</div><div style=\"display:none\">Invisible</div></body></html>";
    let text = html_to_text(html);
    assert!(text.contains("Visible"));
    assert!(!text.contains("Invisible"));
}

#[test]
fn html_to_text_strips_visibility_hidden() {
    let html = "<html><body><div>Visible</div><div style=\"visibility:hidden\">Invisible</div></body></html>";
    let text = html_to_text(html);
    assert!(text.contains("Visible"));
    assert!(!text.contains("Invisible"));
}

#[test]
fn html_to_text_strips_banner_role() {
    let html =
        "<html><body><div role=\"banner\">Cookie banner</div><main>Content</main></body></html>";
    let text = html_to_text(html);
    assert!(!text.contains("Cookie banner"));
    assert!(text.contains("Content"));
}

#[test]
fn html_to_text_strips_script_and_style() {
    let html = "<html><head><style>body { color: red; }</style></head><body><script>console.log('hi');</script><p>Hello</p></body></html>";
    let text = html_to_text(html);
    assert!(!text.contains("color: red"));
    assert!(!text.contains("console.log"));
    assert!(text.contains("Hello"));
}

#[test]
fn detect_charset_from_content_type() {
    let content_type = "text/html; charset=Shift_JIS";
    let bytes = b"<html></html>";
    let encoding = detect_charset(content_type, bytes);
    assert_eq!(encoding.name(), "Shift_JIS");
}

#[test]
fn detect_charset_from_meta_tag() {
    let content_type = "text/html";
    let bytes = b"<html><head><meta charset=\"euc-jp\"></head><body></body></html>";
    let encoding = detect_charset(content_type, bytes);
    assert_eq!(encoding.name(), "EUC-JP");
}

#[test]
fn detect_charset_falls_back_to_utf8() {
    let content_type = "text/html";
    let bytes = b"<html></html>";
    let encoding = detect_charset(content_type, bytes);
    assert_eq!(encoding.name(), "UTF-8");
}

#[test]
fn budget_refund_search_does_not_underflow() {
    let settings = WebSearchSettings {
        allow_multiple_searches: true,
        maximum_searches: 3,
        maximum_page_fetches: 5,
        ..Default::default()
    };
    let mut budget = ToolBudget::new(&settings);
    assert!(budget.take_search());
    assert!(budget.take_search());
    budget.refund_search();
    budget.refund_search(); // Should not underflow
    assert_eq!(budget.searches, 0);
}

#[test]
fn budget_refund_page_does_not_underflow() {
    let settings = WebSearchSettings {
        allow_multiple_searches: true,
        maximum_searches: 3,
        maximum_page_fetches: 5,
        ..Default::default()
    };
    let mut budget = ToolBudget::new(&settings);
    assert!(budget.take_page());
    budget.refund_page();
    budget.refund_page(); // Should not underflow
    assert_eq!(budget.pages, 0);
}

#[test]
fn provider_settings_are_normalized_on_creation() {
    let settings = WebSearchSettings {
        enabled: true,
        allow_multiple_searches: false,
        provider: WebSearchProviderKind::Brave,
        api_key: Some("sk-test".into()),
        tavily_api_key: None,
        exa_api_key: None,
        result_limit: 0,
        request_timeout_seconds: 0,
        maximum_searches: 0,
        maximum_page_fetches: 0,
        minimum_successful_searches: 0,
        minimum_independent_pages: 0,
        tool_iteration_limit: 0,
        custom_research_instructions: String::new(),
    };
    let provider = create_search_provider(&settings);
    // The provider should be created successfully with normalized settings
    // (timeout 0 gets clamped to 3, search/page limit 0 gets clamped to 1)
    assert!(provider.is_ok());
}

#[test]
fn registrable_domain_handles_known_multi_part_tlds() {
    assert_eq!(registrable_domain("www.example.co.uk"), "example.co.uk");
    assert_eq!(registrable_domain("blog.example.com.au"), "example.com.au");
    assert_eq!(registrable_domain("sub.example.co.jp"), "example.co.jp");
    assert_eq!(registrable_domain("example.co.nz"), "example.co.nz");
}

#[test]
fn registrable_domain_falls_back_to_last_two_labels() {
    assert_eq!(registrable_domain("www.example.com"), "example.com");
    assert_eq!(registrable_domain("blog.example.org"), "example.org");
    assert_eq!(registrable_domain("deep.sub.example.io"), "example.io");
}

#[test]
fn registrable_domain_handles_single_label() {
    assert_eq!(registrable_domain("localhost"), "localhost");
    assert_eq!(registrable_domain("myhost"), "myhost");
}
