use crate::app::Correspondence;

/// Keep the original message text, including Markdown/code and reasoning, and
/// represent attachments by name because the clipboard transcript is plain text.
pub fn format_transcript(messages: &[Correspondence], live_reply: Option<(&str, &str)>) -> String {
    let mut turns = Vec::with_capacity(messages.len() + usize::from(live_reply.is_some()));
    for message in messages {
        match message {
            Correspondence::User {
                text,
                images,
                files,
            } => {
                let mut turn = format!("User:\n{text}");
                for image in images {
                    turn.push_str(&format!("\n[Image: {}]", image.name));
                }
                for file in files {
                    turn.push_str(&format!("\n[File: {}]", file.summary()));
                }
                turns.push(turn);
            }
            Correspondence::Bot {
                text,
                model,
                sources,
                ..
            } => {
                let mut turn = assistant_turn(text, model.as_deref());
                if !sources.is_empty() {
                    turn.push_str("\n\nSources:");
                    for source in sources {
                        turn.push_str(&format!("\n- {}: {}", source.title, source.url));
                    }
                }
                turns.push(turn);
            }
        }
    }
    if let Some((text, model)) = live_reply.filter(|(text, _)| !text.trim().is_empty()) {
        turns.push(assistant_turn(text, Some(model)));
    }
    turns.join("\n\n")
}

fn assistant_turn(text: &str, model: Option<&str>) -> String {
    match model.filter(|model| !model.is_empty()) {
        Some(model) => format!("Assistant ({model}):\n{text}"),
        None => format!("Assistant:\n{text}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::web_search::WebSource;

    #[test]
    fn transcript_preserves_order_markdown_reasoning_and_sources() {
        let messages = vec![
            Correspondence::User {
                text: "Explain this 🦀\non two lines".into(),
                images: vec![],
                files: Vec::new(),
            },
            Correspondence::Bot {
                text: "<think>Reasoning</think>Here is the code:\n```rust\nfn main() {}\n```"
                    .into(),
                model: Some("model-a".into()),
                thinking_seconds: None,
                tokens_per_second: None,
                generation_details: None,
                sources: vec![WebSource {
                    title: "Rust".into(),
                    url: "https://www.rust-lang.org".into(),
                }],
                web_search_used: true,
            },
        ];
        assert_eq!(
            format_transcript(&messages, Some(("Partial reply", "model-b"))),
            "User:\nExplain this 🦀\non two lines\n\nAssistant (model-a):\n<think>Reasoning</think>Here is the code:\n```rust\nfn main() {}\n```\n\nSources:\n- Rust: https://www.rust-lang.org\n\nAssistant (model-b):\nPartial reply"
        );
        assert!(format_transcript(&[], Some(("", "model-b"))).is_empty());
        assert_eq!(
            assistant_turn("Older response", None),
            "Assistant:\nOlder response"
        );
    }
}
