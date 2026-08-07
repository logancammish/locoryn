use std::path::Path;

const CONVERSATION_EXCERPT_CHARS: usize = 300;
const DEFAULT_LIMIT: usize = 5;
const MAX_LIMIT: usize = 20;

pub fn tool_definition() -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": "search_locoryn_conversations",
            "description": "Search through the user's previously saved conversations in this app. \
             Use this tool when the user asks about something you discussed before, references a \
             past conversation, asks 'what did we talk about', asks you to recall or find a \
             previous discussion, or when you need context from earlier chats to answer a question. \
             Returns matching conversation titles, dates, and message excerpts containing the query. \
             This searches LOCAL conversation history only — it does not search the web.",
            "parameters": {
                "type": "object",
                "required": ["query"],
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Keywords or phrase to search for in past conversations. Use specific terms the user likely said or discussed."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": MAX_LIMIT,
                        "description": "Maximum number of matching conversations to return. Default is 5."
                    }
                }
            }
        }
    })
}

pub fn parse_limit(arguments: &serde_json::Value) -> usize {
    match arguments.get("limit") {
        Some(value) => value
            .as_u64()
            .and_then(|v| usize::try_from(v).ok())
            .filter(|&v| (1..=MAX_LIMIT).contains(&v))
            .unwrap_or(DEFAULT_LIMIT),
        None => DEFAULT_LIMIT,
    }
}

pub fn search_conversations(
    chat_storage_dir: &Path,
    query: &str,
    limit: usize,
) -> serde_json::Value {
    let chats_path = chat_storage_dir.join("chats.json");
    let data = match std::fs::read_to_string(&chats_path) {
        Ok(data) => data,
        Err(_) => {
            return serde_json::json!({
                "error": "could not read conversation history",
                "matches": 0,
            });
        }
    };
    let chats: Vec<serde_json::Value> = match serde_json::from_str(&data) {
        Ok(chats) => chats,
        Err(_) => {
            return serde_json::json!({
                "error": "could not parse conversation history",
                "matches": 0,
            });
        }
    };

    let query_lower = query.to_lowercase();
    let mut matches = Vec::new();

    for chat in &chats {
        let title = chat
            .get("title")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let updated_at = chat
            .get("updated_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let messages = match chat.get("messages").and_then(serde_json::Value::as_array) {
            Some(msgs) => msgs,
            None => continue,
        };

        let mut matched_excerpts = Vec::new();
        for message in messages {
            let text = match message.get("text").and_then(serde_json::Value::as_str) {
                Some(text) => text,
                None => continue,
            };
            let role = message
                .get("role")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");

            if text.to_lowercase().contains(&query_lower)
                || title.to_lowercase().contains(&query_lower)
            {
                let excerpt = build_excerpt(text, &query_lower, CONVERSATION_EXCERPT_CHARS);
                matched_excerpts.push(serde_json::json!({
                    "role": role,
                    "excerpt": excerpt,
                }));
            }
        }

        if !matched_excerpts.is_empty() {
            matches.push(serde_json::json!({
                "title": title,
                "updated_at": updated_at,
                "matching_messages": matched_excerpts,
            }));
        }
    }

    matches.sort_by(|a, b| {
        let a_date = a
            .get("updated_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        let b_date = b
            .get("updated_at")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        b_date.cmp(a_date)
    });

    let total = matches.len();
    matches.truncate(limit);

    serde_json::json!({
        "matches": total,
        "returned": matches.len(),
        "conversations": matches,
    })
}

fn build_excerpt(text: &str, query_lower: &str, max_chars: usize) -> String {
    let text_lower = text.to_lowercase();
    if let Some(pos) = text_lower.find(query_lower) {
        let query_char_len = query_lower.chars().count();
        let usable = max_chars.saturating_sub(query_char_len.min(max_chars));
        let context_each_side = usable / 2;
        let start = text[..pos]
            .char_indices()
            .rev()
            .take(context_each_side)
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        let query_take = query_char_len.min(max_chars);
        let end = text[pos..]
            .char_indices()
            .take(query_take + context_each_side)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .map(|offset| pos + offset)
            .unwrap_or(text.len());
        let excerpt = text[start..end.min(text.len())].trim();
        let excerpt = excerpt.chars().take(max_chars).collect::<String>();
        if start > 0 {
            format!("...{excerpt}")
        } else {
            excerpt
        }
    } else {
        text.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tool_definition_has_required_fields() {
        let def = tool_definition();
        assert_eq!(def["function"]["name"], "search_locoryn_conversations");
        assert_eq!(
            def["function"]["parameters"]["required"],
            serde_json::json!(["query"])
        );
        assert!(def["function"]["parameters"]["properties"]["query"].is_object());
        assert!(def["function"]["parameters"]["properties"]["limit"].is_object());
    }

    #[test]
    fn parse_limit_defaults_to_five() {
        assert_eq!(parse_limit(&serde_json::json!({})), 5);
    }

    #[test]
    fn parse_limit_clamps_to_valid_range() {
        assert_eq!(parse_limit(&serde_json::json!({"limit": 3})), 3);
        assert_eq!(parse_limit(&serde_json::json!({"limit": 0})), 5);
        assert_eq!(parse_limit(&serde_json::json!({"limit": 25})), 5);
        assert_eq!(parse_limit(&serde_json::json!({"limit": 20})), 20);
    }

    #[test]
    fn search_returns_empty_when_no_chats_file() {
        let dir = std::env::temp_dir().join("locoryn-test-no-chats");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let result = search_conversations(&dir, "test", 5);
        assert_eq!(result["matches"], 0);
        assert!(result["error"].as_str().is_some());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_finds_matching_conversations() {
        let dir = std::env::temp_dir().join("locoryn-test-search");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let chats = serde_json::json!([
            {
                "id": "1",
                "title": "Rust discussion",
                "updated_at": "2025-01-01T00:00:00Z",
                "messages": [
                    {"role": "user", "text": "Tell me about Rust programming"},
                    {"role": "bot", "text": "Rust is a systems programming language"}
                ]
            },
            {
                "id": "2",
                "title": "Python chat",
                "updated_at": "2025-01-02T00:00:00Z",
                "messages": [
                    {"role": "user", "text": "How about Python?"},
                    {"role": "bot", "text": "Python is great for scripting"}
                ]
            }
        ]);
        fs::write(dir.join("chats.json"), chats.to_string()).unwrap();

        let result = search_conversations(&dir, "rust", 5);
        assert_eq!(result["matches"], 1);
        assert_eq!(result["conversations"][0]["title"], "Rust discussion");

        let result = search_conversations(&dir, "python", 5);
        assert_eq!(result["matches"], 1);
        assert_eq!(result["conversations"][0]["title"], "Python chat");

        let result = search_conversations(&dir, "nonexistent", 5);
        assert_eq!(result["matches"], 0);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_is_case_insensitive() {
        let dir = std::env::temp_dir().join("locoryn-test-case");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let chats = serde_json::json!([
            {
                "id": "1",
                "title": "UPPERCASE Title",
                "updated_at": "2025-01-01T00:00:00Z",
                "messages": [
                    {"role": "user", "text": "Some CONTENT here"}
                ]
            }
        ]);
        fs::write(dir.join("chats.json"), chats.to_string()).unwrap();

        let result = search_conversations(&dir, "uppercase", 5);
        assert_eq!(result["matches"], 1);

        let result = search_conversations(&dir, "CONTENT", 5);
        assert_eq!(result["matches"], 1);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_respects_limit() {
        let dir = std::env::temp_dir().join("locoryn-test-limit");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let chats = serde_json::json!([
            {"id": "1", "title": "Chat about topic", "updated_at": "2025-01-01T00:00:00Z", "messages": [{"role": "user", "text": "topic"}]},
            {"id": "2", "title": "Another topic chat", "updated_at": "2025-01-02T00:00:00Z", "messages": [{"role": "user", "text": "topic"}]},
            {"id": "3", "title": "Third topic chat", "updated_at": "2025-01-03T00:00:00Z", "messages": [{"role": "user", "text": "topic"}]}
        ]);
        fs::write(dir.join("chats.json"), chats.to_string()).unwrap();

        let result = search_conversations(&dir, "topic", 2);
        assert_eq!(result["matches"], 3);
        assert_eq!(result["returned"], 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_sorts_by_most_recent_first() {
        let dir = std::env::temp_dir().join("locoryn-test-sort");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let chats = serde_json::json!([
            {"id": "1", "title": "Old", "updated_at": "2024-01-01T00:00:00Z", "messages": [{"role": "user", "text": "match"}]},
            {"id": "2", "title": "New", "updated_at": "2025-06-01T00:00:00Z", "messages": [{"role": "user", "text": "match"}]},
            {"id": "3", "title": "Mid", "updated_at": "2024-06-01T00:00:00Z", "messages": [{"role": "user", "text": "match"}]}
        ]);
        fs::write(dir.join("chats.json"), chats.to_string()).unwrap();

        let result = search_conversations(&dir, "match", 5);
        assert_eq!(result["conversations"][0]["title"], "New");
        assert_eq!(result["conversations"][1]["title"], "Mid");
        assert_eq!(result["conversations"][2]["title"], "Old");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn build_excerpt_centers_on_match() {
        let text = "short text";
        let excerpt = build_excerpt(text, "text", 300);
        assert!(excerpt.contains("text"));
    }

    #[test]
    fn build_excerpt_truncates_long_text() {
        let text = "a".repeat(1000);
        let excerpt = build_excerpt(&text, &"a".repeat(1000), 100);
        assert!(excerpt.chars().count() <= 100);
    }
}
