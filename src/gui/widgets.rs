use super::*;

pub(super) fn standard_reasoning_slider_available(levels: &[ThinkingLevel]) -> bool {
    [
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
    ]
    .iter()
    .all(|level| levels.contains(level))
}

fn reasoning_slider_style(_theme: &Theme, status: widget::slider::Status) -> widget::slider::Style {
    let active = matches!(status, widget::slider::Status::Dragged);
    widget::slider::Style {
        rail: widget::slider::Rail {
            backgrounds: (
                Background::Color(accent()),
                Background::Color(if is_dark_mode() {
                    border_bright()
                } else {
                    border_soft()
                }),
            ),
            width: 7.0,
            border: Border {
                radius: Radius::from(8.0),
                ..Border::default()
            },
        },
        handle: widget::slider::Handle {
            shape: widget::slider::HandleShape::Circle {
                radius: if active { 10.0 } else { 9.0 },
            },
            background: Background::Color(if is_dark_mode() {
                text_main()
            } else {
                Color::WHITE
            }),
            border_width: 2.0,
            border_color: control_accent(),
        },
    }
}

fn reasoning_slider<'a>(
    selected: ThinkingLevel,
    levels: &[ThinkingLevel],
    language: Language,
    width: Length,
    show_labels: bool,
) -> Element<'a, Message> {
    let options = levels.to_vec();
    let selected_index = options
        .iter()
        .position(|level| *level == selected)
        .unwrap_or(0);
    let maximum = options.len().saturating_sub(1) as f32;
    let slider_options = options.clone();
    let slider = widget::slider(0.0..=maximum, selected_index as f32, move |value| {
        let index = (value.round() as usize).min(slider_options.len().saturating_sub(1));
        Message::ThinkingLevelChange(slider_options[index])
    })
    .step(1.0_f32)
    .width(width)
    .style(reasoning_slider_style);

    if !show_labels {
        return slider.into();
    }

    let labels = options
        .iter()
        .map(|level| {
            let selected_label = *level == selected;
            container(
                widget::text(
                    ThinkingChoice {
                        level: *level,
                        language,
                    }
                    .to_string(),
                )
                .size(10)
                .color(if selected_label {
                    control_accent()
                } else {
                    text_faint()
                })
                .align_x(Horizontal::Center),
            )
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
        })
        .collect::<Vec<Element<'a, Message>>>();

    widget::column![
        slider,
        Space::new().height(Length::Fixed(4.0)),
        widget::Row::with_children(labels).width(Length::Fill),
    ]
    .into()
}

pub(super) fn compact_thinking_control<'a>(
    selected: ThinkingLevel,
    levels: &[ThinkingLevel],
    language: Language,
) -> Element<'a, Message> {
    let label = ThinkingChoice {
        level: selected,
        language,
    }
    .to_string();
    let control = widget::column![
        widget::text(label)
            .size(10)
            .color(control_accent())
            .align_x(Horizontal::Center)
            .width(Length::Fill),
        reasoning_slider(selected, levels, language, Length::Fill, false),
    ]
    .spacing(iced::Pixels(2.0));

    widget::tooltip(
        container(control)
            .padding([3, 8])
            .width(Length::Fixed(176.0))
            .style(flat_card_style),
        container(
            widget::text(tr(language, "Reasoning effort"))
                .size(11)
                .color(text_main()),
        )
        .padding(8)
        .style(flat_card_style),
        widget::tooltip::Position::Bottom,
    )
    .into()
}

pub(super) fn thinking_control<'a>(
    selected: ThinkingLevel,
    levels: &[ThinkingLevel],
    language: Language,
) -> Element<'a, Message> {
    let description = match selected {
        ThinkingLevel::Off => "Fastest · no extra reasoning",
        ThinkingLevel::On => "Use this model's standard reasoning mode",
        ThinkingLevel::Minimal => "Minimal reasoning for very quick responses",
        ThinkingLevel::Low => "Quick reasoning for everyday questions",
        ThinkingLevel::Medium => "Balanced for multi-step tasks",
        ThinkingLevel::High => "Most thorough · slower responses",
        ThinkingLevel::XHigh => "Extra-deep reasoning for difficult tasks",
        ThinkingLevel::Max => "Maximum reasoning the model offers",
    };

    let selector: Element<'a, Message> = if standard_reasoning_slider_available(levels) {
        reasoning_slider(selected, levels, language, Length::Fill, true)
    } else {
        widget::pick_list(
            ThinkingChoice::from_levels(levels, language),
            Some(ThinkingChoice {
                level: selected,
                language,
            }),
            |choice| Message::ThinkingLevelChange(choice.level),
        )
        .placeholder(tr(language, "Thinking"))
        .padding([12, 14])
        .text_size(14)
        .style(pick_list_style)
        .menu_style(pick_list_menu_style)
        .width(Length::Fill)
        .into()
    };

    widget::column![
        selector,
        Space::new().height(Length::Fixed(6.0)),
        widget::text(tr(language, description))
            .size(11)
            .color(text_faint()),
    ]
    .into()
}

pub(super) fn image_preview<'a>(
    image: &ChatImage,
    remove_index: Option<usize>,
    language: Language,
) -> Element<'a, Message> {
    let preview = widget::image(image.preview_handle.clone())
        .width(Length::Fixed(160.0))
        .height(Length::Fixed(110.0))
        .content_fit(iced::ContentFit::Contain)
        .border_radius(10.0);
    let footer: Element<Message> = if let Some(index) = remove_index {
        widget::row![
            widget::text(ellipsize_chat_title(&image.name, 16))
                .size(11)
                .color(text_muted())
                .wrapping(Wrapping::None),
            Space::new().width(Length::Fill),
            mini_button(tr(language, "Remove"), Message::RemoveImage(index)),
        ]
        .into()
    } else {
        widget::text(ellipsize_chat_title(
            &format!("{} · {}", image.name, image.mime_type),
            22,
        ))
        .size(11)
        .color(text_muted())
        .wrapping(Wrapping::None)
        .into()
    };
    container(widget::column![
        preview,
        Space::new().height(Length::Fixed(6.0)),
        footer
    ])
    .padding(8)
    .width(Length::Fixed(176.0))
    .clip(true)
    .style(flat_card_style)
    .into()
}

pub(super) fn image_previews<'a>(
    images: &[ChatImage],
    removable: bool,
    language: Language,
) -> Element<'a, Message> {
    if images.is_empty() {
        return widget::column![].into();
    }

    let previews = widget::Row::with_children(
        images
            .iter()
            .enumerate()
            .map(|(index, image)| image_preview(image, removable.then_some(index), language))
            .collect::<Vec<_>>(),
    )
    .spacing(iced::Pixels(6.0));

    widget::scrollable(previews)
        .direction(widget::scrollable::Direction::Horizontal(
            widget::scrollable::Scrollbar::default()
                .width(5.0)
                .scroller_width(5.0),
        ))
        .height(Length::Fixed(172.0))
        .into()
}

pub(super) fn composer_image_preview<'a>(image: &ChatImage, index: usize) -> Element<'a, Message> {
    let preview = widget::image(image.preview_handle.clone())
        .width(Length::Fixed(46.0))
        .height(Length::Fixed(46.0))
        .content_fit(iced::ContentFit::Cover)
        .border_radius(8.0);
    let remove = widget::button(
        widget::text("×")
            .size(14)
            .color(text_muted())
            .align_x(Horizontal::Center),
    )
    .padding([3, 6])
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_muted(), status))
    .on_press(Message::RemoveImage(index));

    container(
        widget::row![preview, Space::new().width(Length::Fixed(4.0)), remove,]
            .align_y(iced::Alignment::Center),
    )
    .padding(5)
    .width(Length::Fixed(88.0))
    .height(Length::Fixed(56.0))
    .style(flat_card_style)
    .into()
}

pub(super) fn composer_image_previews<'a>(images: &[ChatImage]) -> Element<'a, Message> {
    let previews = widget::Row::with_children(
        images
            .iter()
            .enumerate()
            .map(|(index, image)| composer_image_preview(image, index))
            .collect::<Vec<_>>(),
    )
    .spacing(iced::Pixels(6.0));
    let rail_width = (images.len() as f32 * 94.0).min(212.0);

    widget::scrollable(previews)
        .direction(widget::scrollable::Direction::Horizontal(
            widget::scrollable::Scrollbar::default()
                .width(4.0)
                .scroller_width(4.0),
        ))
        .width(Length::Fixed(rail_width))
        .height(Length::Fixed(62.0))
        .into()
}

pub(super) fn copy_code_button<'a>(
    copied: bool,
    language: Language,
    message: Message,
) -> Element<'a, Message> {
    let label = tr(language, if copied { "Copied ✓" } else { "Copy code" });

    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding(8)
        .style(move |_theme, status| {
            if copied {
                button_visual(rgb(31, 92, 63), rgb(93, 225, 144), Color::WHITE, status)
            } else {
                button_visual(panel_soft(), border_soft(), text_muted(), status)
            }
        })
        .on_press(message)
        .into()
}

pub(super) fn text_input_style(
    _theme: &Theme,
    status: widget::text_input::Status,
) -> widget::text_input::Style {
    let focused = matches!(status, widget::text_input::Status::Focused { .. });
    let hovered = matches!(status, widget::text_input::Status::Hovered);
    widget::text_input::Style {
        background: Background::Color(panel_soft()),
        border: Border {
            color: if focused {
                accent()
            } else if hovered {
                border_bright()
            } else {
                border_soft()
            },
            width: if focused { 1.5 } else { 1.0 },
            radius: Radius::from(12.0),
        },
        icon: text_muted(),
        placeholder: text_faint(),
        value: text_main(),
        selection: accent(),
    }
}

pub(super) fn feedback_text_input_style(
    bounce: f32,
) -> impl Fn(&Theme, widget::text_input::Status) -> widget::text_input::Style {
    move |theme, status| {
        let mut style = text_input_style(theme, status);
        style.background =
            Background::Color(mix_color(panel_soft(), control_accent(), bounce * 0.05));
        style.border.color = mix_color(style.border.color, control_accent(), bounce * 0.35);
        style.border.width += bounce * 0.55;
        style
    }
}

pub(super) fn text_editor_style(
    _theme: &Theme,
    status: widget::text_editor::Status,
) -> widget::text_editor::Style {
    let focused = matches!(status, widget::text_editor::Status::Focused { .. });
    let hovered = matches!(status, widget::text_editor::Status::Hovered);
    widget::text_editor::Style {
        background: Background::Color(panel_soft()),
        border: Border {
            color: if focused {
                accent()
            } else if hovered {
                border_bright()
            } else {
                border_soft()
            },
            width: if focused { 1.5 } else { 1.0 },
            radius: Radius::from(12.0),
        },
        placeholder: text_faint(),
        value: text_main(),
        selection: accent(),
    }
}

pub(super) fn section_title<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    widget::column![
        widget::text(title).size(27).color(text_main()),
        Space::new().height(Length::Fixed(5.0)),
        widget::text(subtitle).size(14).color(text_muted()),
    ]
    .into()
}

pub(super) fn setting_label<'a>(title: &'a str, subtitle: &'a str) -> Element<'a, Message> {
    widget::column![
        widget::text(title).size(16).color(text_main()),
        Space::new().height(Length::Fixed(4.0)),
        widget::text(subtitle).size(12).color(text_muted()),
    ]
    .width(Length::Fill)
    .into()
}

pub(super) fn settings_group_title<'a>(title: &'a str) -> Element<'a, Message> {
    widget::row![
        widget::text(title).size(11).color(control_accent()),
        Space::new().width(Length::Fixed(10.0)),
        widget::rule::horizontal(1).style(|_theme| widget::rule::Style {
            color: border_soft(),
            radius: Radius::from(1.0),
            fill_mode: widget::rule::FillMode::Full,
            snap: true,
        }),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

pub(super) fn help_card<'a>(title: &'a str, body: &'a str, color: Color) -> Element<'a, Message> {
    container(widget::column![
        container(widget::text(title).size(16).color(text_main()))
            .padding(8)
            .style(chip_style(color)),
        Space::new().height(Length::Fixed(10.0)),
        widget::text(body).size(14).color(text_muted()),
    ])
    .padding(16)
    .width(Length::Fill)
    .style(flat_card_style)
    .into()
}

pub(super) fn website_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_string))
        .map(|host| host.strip_prefix("www.").unwrap_or(&host).to_string())
        .unwrap_or_else(|| url.to_string())
}

pub(super) fn ellipsize_chat_title(title: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }

    let single_line = title.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = single_line.chars();
    let mut shortened = chars.by_ref().take(max_chars).collect::<String>();

    if chars.next().is_some() {
        shortened.pop();
        shortened.push('…');
    }

    shortened
}

pub(super) fn website_result_row(
    index: usize,
    source: &WebSource,
    active: bool,
    language: Language,
) -> Element<'static, Message> {
    let host = website_host(&source.url);
    let title = ellipsize_chat_title(&source.title, 64);
    let host = ellipsize_chat_title(&host, 64);
    let trailing = if active {
        tr(language, "READING").to_string()
    } else {
        "↗".to_string()
    };
    let trailing_color = if active { accent_2() } else { text_faint() };

    container(
        widget::button(widget::row![
            container(
                widget::text((index + 1).to_string())
                    .size(11)
                    .color(if active { accent_2() } else { text_muted() })
                    .align_x(Horizontal::Center)
            )
            .padding([4, 7])
            .style(chip_style(if active {
                accent_2()
            } else {
                border_bright()
            })),
            Space::new().width(Length::Fixed(9.0)),
            widget::column![
                widget::text(title)
                    .size(12)
                    .color(text_main())
                    .wrapping(Wrapping::None),
                Space::new().height(Length::Fixed(2.0)),
                widget::text(host)
                    .size(11)
                    .color(text_muted())
                    .wrapping(Wrapping::None),
            ]
            .width(Length::Fill),
            widget::text(trailing).size(10).color(trailing_color),
        ])
        .on_press(Message::OpenSource(source.url.clone()))
        .padding(0)
        .style(chat_title_button_style)
        .clip(true)
        .width(Length::Fill),
    )
    .padding(9)
    .width(Length::Fill)
    .style(website_row_style(active))
    .into()
}

pub(super) fn web_search_activity<'a>(
    state: WebSearchState,
    language: Language,
    web_search_enabled: bool,
) -> Element<'a, Message> {
    // This is the final rendering gate. Local tools also use the shared tool
    // loop and its synthesizing state, but must never present web-source UI.
    if !web_search_activity_visible(web_search_enabled, &state) {
        return widget::column![].into();
    }
    let (status, detail, websites, active_url, status_color) = match state {
        WebSearchState::Searching { query, websites } => (
            tr(language, "Searching the web…"),
            query,
            websites,
            None,
            accent_2(),
        ),
        WebSearchState::Results { query, websites } => (
            tr(language, "Reviewing results"),
            query,
            websites,
            None,
            warning(),
        ),
        WebSearchState::Fetching {
            url,
            query,
            websites,
        } => (
            tr(language, "Reading website"),
            if query.trim().is_empty() {
                website_host(&url)
            } else {
                query
            },
            websites,
            Some(url),
            success(),
        ),
        WebSearchState::Synthesizing {
            query, websites, ..
        } => (
            tr(language, "Preparing the answer from these sources."),
            if query.trim().is_empty() {
                tr(language, "The model is choosing which result to read.").to_string()
            } else {
                query
            },
            websites,
            None,
            accent_2(),
        ),
        WebSearchState::Failed { message } => (
            tr(language, "Web search"),
            message,
            Vec::new(),
            None,
            danger(),
        ),
        WebSearchState::Idle | WebSearchState::Completed => {
            return widget::column![].into();
        }
    };
    let detail = ellipsize_chat_title(&detail, 72);
    let result_count: Element<'a, Message> = if websites.is_empty() {
        widget::column![].into()
    } else {
        let count = websites.len();
        container(
            widget::text(format!("{count} {}", tr(language, "found")))
                .size(10)
                .color(status_color),
        )
        .padding([4, 7])
        .style(chip_style(status_color))
        .into()
    };
    let header: Element<'a, Message> = container(widget::row![
        widget::text("●").size(10).color(status_color),
        Space::new().width(Length::Fixed(7.0)),
        widget::text(status)
            .size(12)
            .color(status_color)
            .wrapping(Wrapping::None),
        Space::new().width(Length::Fixed(9.0)),
        widget::text(detail)
            .size(12)
            .color(text_muted())
            .wrapping(Wrapping::None)
            .width(Length::Fill),
        result_count,
    ])
    .padding([8, 10])
    .width(Length::Fill)
    .clip(true)
    .style(web_activity_style)
    .into();
    let result_rows = websites
        .iter()
        .enumerate()
        .take(10)
        .map(|(index, source)| {
            website_result_row(
                index,
                source,
                active_url.as_deref() == Some(source.url.as_str()),
                language,
            )
        })
        .collect::<Vec<_>>();
    let results: Element<'a, Message> = if result_rows.is_empty() {
        widget::column![].into()
    } else {
        container(widget::Column::with_children(result_rows).spacing(iced::Pixels(4.0)))
            .padding([6, 0])
            .width(Length::Fill)
            .into()
    };

    widget::column![header, results,].into()
}

pub(super) fn web_search_activity_visible(
    web_search_enabled: bool,
    state: &WebSearchState,
) -> bool {
    web_search_enabled && !matches!(state, WebSearchState::Idle | WebSearchState::Completed)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn markdown_with_code_copy<'a>(
    items: &'a [markdown::Item],
    code_copy_scope: CodeCopyScope,
    highlight_code_blocks: bool,
    text_size: f32,
    font_family: FontFamily,
    copied_text: Option<&String>,
    language: Language,
    code_checking_enabled: bool,
    markdown_images: &'a std::collections::HashMap<String, MarkdownImageState>,
    motion: f32,
) -> Element<'a, Message> {
    // Iced's Markdown parser precomputes highlighted lines. Live snapshots omit
    // those lines and deliberately use this plain code view; completed messages
    // are parsed with highlighting once. Very large completed snippets stay
    // capped too.
    const MAX_HIGHLIGHTED_CODE_BYTES: usize = 4 * 1024;
    const MAX_CODE_PREVIEW_BYTES: usize = 8 * 1024;

    let mut settings = iced::widget::markdown::Settings::with_text_size(
        text_size,
        if is_dark_mode() {
            Theme::Dark
        } else {
            Theme::Light
        },
    );
    settings.style.font = chat_font(font_family);

    let mut children: Vec<Element<'a, Message>> = Vec::new();
    let mut code_block_index = 0;
    let mut highlighted_code_bytes = 0_usize;

    for item in items.iter() {
        if let markdown::Item::Image { url, .. } = item {
            let image_block: Element<'a, Message> = match markdown_images.get(url) {
                Some(MarkdownImageState::Ready(handle)) => container(widget::column![
                    widget::image(handle.clone())
                        .width(Length::Fill)
                        .height(Length::Fixed(320.0))
                        .content_fit(iced::ContentFit::Contain)
                        .border_radius(12.0),
                    Space::new().height(Length::Fixed(7.0)),
                    widget::text(ellipsize_chat_title(&website_host(url), 64))
                        .size(11)
                        .color(text_muted())
                        .wrapping(Wrapping::None),
                ])
                .padding(10)
                .width(Length::Fill)
                .max_width(720)
                .style(flat_card_style)
                .into(),
                Some(MarkdownImageState::Failed(error)) => container(widget::row![
                    widget::text("!")
                        .size(12)
                        .color(danger())
                        .align_x(Horizontal::Center),
                    Space::new().width(Length::Fixed(8.0)),
                    widget::column![
                        widget::text("Image unavailable")
                            .size(12)
                            .color(text_main()),
                        widget::text(ellipsize_chat_title(error, 88))
                            .size(11)
                            .color(text_muted()),
                    ],
                ])
                .padding([10, 12])
                .width(Length::Fill)
                .style(chip_style(danger()))
                .into(),
                Some(MarkdownImageState::Loading) | None => {
                    let dots = ".".repeat(((motion * 4.0) as usize % 3) + 1);
                    container(widget::row![
                        widget::text("○").size(13).color(accent_2()),
                        Space::new().width(Length::Fixed(8.0)),
                        widget::text(format!("Loading image{dots}"))
                            .size(12)
                            .color(text_muted()),
                    ])
                    .padding([10, 12])
                    .width(Length::Fill)
                    .style(chip_style(accent_2()))
                    .into()
                }
            };
            children.push(image_block);
            continue;
        }

        if let markdown::Item::CodeBlock {
            language: code_language,
            code,
            ..
        } = item
        {
            let use_plain_preview = !highlight_code_blocks
                || code.len() > MAX_HIGHLIGHTED_CODE_BYTES
                || highlighted_code_bytes.saturating_add(code.len()) > MAX_HIGHLIGHTED_CODE_BYTES;
            if use_plain_preview {
                let mut preview_end = code.len().min(MAX_CODE_PREVIEW_BYTES);
                while !code.is_char_boundary(preview_end) {
                    preview_end -= 1;
                }
                let preview = if preview_end == code.len() {
                    code.clone()
                } else {
                    let omitted = cached_character_count(code)
                        .saturating_sub(cached_character_count(&code[..preview_end]));
                    format!(
                        "{}\n\n… {omitted} characters omitted from this preview. Copy code to get the complete snippet.",
                        &code[..preview_end]
                    )
                };
                children.push(
                    container(
                        widget::scrollable(
                            widget::text(preview)
                                .size(text_size - 1.0)
                                .font(iced::Font::MONOSPACE)
                                .wrapping(Wrapping::None),
                        )
                        .height(Length::Fixed(320.0)),
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into(),
                );
            } else {
                highlighted_code_bytes += code.len();
                children.push(
                    selectable_markdown(std::iter::once(item), settings).map(|_| Message::None),
                );
            }

            let copied = copied_text.map(|copied| copied == code).unwrap_or(false);
            let check_button: Element<'a, Message> = code_language
                .as_deref()
                .and_then(crate::canonical_code_language)
                .filter(|_| code_checking_enabled)
                .map(|canonical| {
                    mini_button_owned(
                        tr(language, "Check code").to_string(),
                        Message::CheckCode(
                            code_copy_scope,
                            code_block_index,
                            canonical.to_string(),
                        ),
                    )
                })
                .unwrap_or_else(|| widget::column![].into());

            children.push(
                widget::row![
                    Space::new().width(Length::Fill),
                    check_button,
                    Space::new().width(Length::Fixed(6.0)),
                    copy_code_button(
                        copied,
                        language,
                        Message::CopyCode(code_copy_scope, code_block_index),
                    ),
                ]
                .into(),
            );
            code_block_index += 1;
        } else {
            children
                .push(selectable_markdown(std::iter::once(item), settings).map(|_| Message::None));
        }
    }

    widget::Column::with_children(children)
        .spacing(iced::Pixels(8.0))
        .into()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn message_bubble<'a>(
    index: usize,
    message: &'a Correspondence,
    parsed_markdown: Option<&'a [markdown::Item]>,
    cached_thinking: &'a str,
    text_size: f32,
    font_family: FontFamily,
    model_name: String,
    copied_text: Option<&String>,
    thinking_expanded: bool,
    sources_expanded: bool,
    language: Language,
    code_checking_enabled: bool,
    show_tokens_per_second: bool,
    markdown_images: &'a std::collections::HashMap<String, MarkdownImageState>,
    reveal: f32,
    motion: f32,
) -> Element<'a, Message> {
    match message {
        Correspondence::User { text, images } => widget::row![
            Space::new().width(Length::Fill),
            container(widget::column![
                widget::text(tr(language, "You"))
                    .size(12)
                    .color(if is_dark_mode() {
                        rgb(205, 221, 255)
                    } else {
                        rgb(55, 72, 150)
                    })
                    .align_x(Horizontal::Right),
                Space::new().height(Length::Fixed(6.0)),
                image_previews(images, false, language),
                widget::text(text)
                    .size(text_size)
                    .font(chat_font(font_family))
                    .align_x(Horizontal::Right),
            ])
            .padding([13, 16])
            .width(Length::Shrink)
            .max_width(760)
            .style(user_bubble_style(reveal)),
            Space::new().width(Length::Fixed(4.0 + (1.0 - reveal) * 8.0)),
        ]
        .into(),

        Correspondence::Bot {
            text,
            thinking_seconds,
            tokens_per_second,
            generation_details,
            sources,
            web_search_used,
            ..
        } => {
            let body: Element<'a, Message> = if let Some(parsed) = parsed_markdown {
                markdown_with_code_copy(
                    parsed,
                    CodeCopyScope::ChatMessage(index),
                    true,
                    text_size,
                    font_family,
                    copied_text,
                    language,
                    code_checking_enabled,
                    markdown_images,
                    motion,
                )
            } else {
                let (_, fallback_text) = split_thinking_text(text);
                widget::text(fallback_text)
                    .size(text_size)
                    .font(chat_font(font_family))
                    .color(text_main())
                    .align_x(Horizontal::Left)
                    .into()
            };

            let reasoning: Element<'a, Message> = if cached_thinking.is_empty() {
                widget::column![].into()
            } else {
                let label = if let Some(seconds) = thinking_seconds {
                    if thinking_expanded {
                        format!(
                            "▾ {}",
                            if language == Language::Spanish {
                                format!("Razonó durante {seconds} segundos")
                            } else {
                                format!("Thought for {seconds} seconds")
                            }
                        )
                    } else if language == Language::Spanish {
                        format!("▸ Razonó durante {seconds} segundos")
                    } else {
                        format!("▸ Thought for {seconds} seconds")
                    }
                } else if thinking_expanded {
                    tr(language, "▾ Hide thinking").to_string()
                } else {
                    tr(language, "▸ Show thinking").to_string()
                };
                let details: Element<'a, Message> = if thinking_expanded {
                    container(
                        widget::text(cached_thinking)
                            .size(text_size - 1.0)
                            .font(chat_font(font_family))
                            .color(text_muted()),
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                } else {
                    widget::column![].into()
                };
                widget::column![
                    mini_button_owned(label, Message::ToggleThinking(index)),
                    details,
                    Space::new().height(Length::Fixed(7.0)),
                ]
                .into()
            };

            let source_list: Element<'a, Message> = if sources.is_empty() {
                widget::column![].into()
            } else {
                let label = if sources_expanded {
                    format!("▾ {} ({})", tr(language, "Sources"), sources.len())
                } else {
                    format!("▸ {} ({})", tr(language, "Sources"), sources.len())
                };
                let entries: Element<'a, Message> = if sources_expanded {
                    let entries = sources
                        .iter()
                        .enumerate()
                        .map(|(source_index, source)| {
                            website_result_row(source_index, source, false, language)
                        })
                        .collect::<Vec<Element<'a, Message>>>();
                    widget::column![
                        Space::new().height(Length::Fixed(5.0)),
                        widget::Column::with_children(entries).spacing(iced::Pixels(5.0)),
                    ]
                    .into()
                } else {
                    widget::column![].into()
                };
                widget::column![
                    Space::new().height(Length::Fixed(12.0)),
                    mini_button_owned(label, Message::ToggleSources(index)),
                    entries,
                ]
                .into()
            };

            // Generation telemetry is deliberately split into rate/count and
            // timing rows. This explains what the headline tok/s value covers
            // without making the reply header noisy.
            let speed_note: Element<'a, Message> = if show_tokens_per_second {
                let mut counts = Vec::<String>::new();
                let mut timings = Vec::<String>::new();
                if let Some(tps) = tokens_per_second {
                    counts.push(format!("{tps:.1} tok/s"));
                }
                if let Some(details) = generation_details {
                    if let Some(tokens) = details.output_tokens {
                        counts.push(format!("{tokens} {}", tr(language, "generated")));
                    }
                    if let Some(tokens) = details.prompt_tokens {
                        counts.push(format!("{tokens} {}", tr(language, "prompt tokens")));
                    }
                    let format_seconds = |milliseconds: u64| {
                        let seconds = milliseconds as f64 / 1_000.0;
                        if seconds < 10.0 {
                            format!("{seconds:.2}s")
                        } else {
                            format!("{seconds:.1}s")
                        }
                    };
                    if let Some(duration) = details.generation_duration_ms {
                        let local = if details.generation_duration_locally_measured {
                            format!(" · {}", tr(language, "client timed"))
                        } else {
                            String::new()
                        };
                        timings.push(format!(
                            "{} {}{local}",
                            tr(language, "Generation"),
                            format_seconds(duration),
                        ));
                    }
                    if let Some(duration) = details.time_to_first_token_ms {
                        timings.push(format!(
                            "{} {}",
                            tr(language, "First token"),
                            format_seconds(duration),
                        ));
                    }
                    if let Some(duration) = details.prompt_duration_ms {
                        timings.push(format!(
                            "{} {}",
                            tr(language, "Prompt processing"),
                            format_seconds(duration),
                        ));
                    }
                    if let Some(duration) =
                        details.load_duration_ms.filter(|duration| *duration > 0)
                    {
                        timings.push(format!(
                            "{} {}",
                            tr(language, "Model load"),
                            format_seconds(duration),
                        ));
                    }
                    if let Some(duration) = details.response_duration_ms {
                        timings.push(format!(
                            "{} {}",
                            tr(language, "Response total"),
                            format_seconds(duration),
                        ));
                    }
                }

                if counts.is_empty() && timings.is_empty() {
                    widget::column![].into()
                } else {
                    let mut lines = widget::Column::new().spacing(iced::Pixels(2.0));
                    if !counts.is_empty() {
                        lines = lines.push(
                            widget::text(counts.join(" · "))
                                .size(11)
                                .color(text_muted()),
                        );
                    }
                    if !timings.is_empty() {
                        lines = lines.push(
                            widget::text(timings.join(" · "))
                                .size(10)
                                .color(text_faint()),
                        );
                    }
                    widget::column![
                        Space::new().height(Length::Fixed(10.0)),
                        widget::tooltip(
                            lines,
                            container(
                                widget::text(tr(
                                    language,
                                    "tok/s measures output generation only. Response total includes prompt processing, model loading, tools, and web research. OpenVINO timing is measured by Locoryn when the server omits a duration."
                                ))
                                .size(11)
                                .color(text_main())
                            )
                            .padding(9)
                            .max_width(420)
                            .style(flat_card_style),
                            widget::tooltip::Position::Bottom,
                        )
                    ]
                    .into()
                }
            } else {
                widget::column![].into()
            };

            widget::row![
                Space::new().width(Length::Fixed((1.0 - reveal) * 8.0)),
                container(widget::text("✦").size(17).color(Color::WHITE))
                    .padding([8, 11])
                    .style(assistant_mark_style(motion, false)),
                Space::new().width(Length::Fixed(10.0)),
                container(widget::column![
                    widget::row![
                        widget::text(model_name).size(12).color(accent_2()),
                        Space::new().width(Length::Fill),
                        // Older chats may have recorded this flag whenever the tool loop
                        // ran, including for local-only tools. A source is the evidence that
                        // a response genuinely relied on the web.
                        if *web_search_used && !sources.is_empty() {
                            container(widget::text(tr(language, "WEB")).size(10).color(success()))
                                .padding([4, 7])
                                .style(chip_style(success()))
                        } else {
                            container(widget::text("")).padding(0)
                        },
                        Space::new().width(Length::Fixed(6.0)),
                        mini_button(tr(language, "Copy response"), Message::CopyResponse(index)),
                    ],
                    Space::new().height(Length::Fixed(7.0)),
                    reasoning,
                    body,
                    source_list,
                    speed_note,
                ])
                .padding(14)
                .width(Length::Fill)
                .max_width(920)
                .style(bot_bubble_style(reveal)),
                Space::new().width(Length::Fixed(24.0)),
            ]
            .into()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{standard_reasoning_slider_available, web_search_activity_visible};
    use crate::{ThinkingLevel, tools::web_search::WebSearchState};

    #[test]
    fn local_tool_synthesis_never_shows_web_search_activity() {
        let state = WebSearchState::Synthesizing {
            thinking: "Checking code".to_string(),
            query: String::new(),
            websites: Vec::new(),
        };

        assert!(!web_search_activity_visible(false, &state));
        assert!(web_search_activity_visible(true, &state));
    }

    #[test]
    fn reasoning_slider_requires_low_medium_and_high() {
        assert!(standard_reasoning_slider_available(&[
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ]));
        assert!(!standard_reasoning_slider_available(&[
            ThinkingLevel::Low,
            ThinkingLevel::High,
        ]));
    }
}
