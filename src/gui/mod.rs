use iced::{
    Background, Border, Color, Element, Length, Shadow, Theme, Vector,
    alignment::Horizontal,
    border::Radius,
    mouse,
    widget::{self, Space, container},
};

use iced_selection::markdown as selectable_markdown;
use iced_widget::{container::Style, core::text::Wrapping, markdown};
use std::{
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};

use crate::{
    AppUpdateState, ChatImage, Correspondence, FontFamily, GUIState, Language, MarkdownImageState,
    Message, Program, SettingsFeedbackTarget, ThinkingLevel, split_thinking_text,
    tools::web_search::{WebSearchState, WebSource},
};

mod theme;
mod translation;
mod widgets;

use theme::*;
use translation::*;
use widgets::*;

pub(crate) use theme::set_dark_mode;

impl Program {
    pub fn get_ui_information<'a>(
        &'a self,
        gui_state: &'a GUIState,
    ) -> iced::widget::Container<'a, Message> {
        let language = self.user_information.language;
        match gui_state {
            GUIState::InfoPopup => {
                let content = container(
                    widget::column![
                        Space::new().height(Length::Fixed(
                            (1.0 - eased(self.page_reveal)) * 4.0
                        )),
                        container(
                            widget::row![
                                widget::column![
                                    widget::text("Locoryn")
                                        .size(30)
                                        .color(text_main()),
                                    Space::new().height(Length::Fixed(6.0)),
                                    widget::text(tr(language, "A polished desktop interface for chatting with local Ollama models."))
                                        .size(14)
                                        .color(text_muted()),
                                ]
                                .width(Length::Fill),

                                container(
                                    widget::text(tr(language, "HELP"))
                                        .size(13)
                                        .color(text_main())
                                )
                                .padding(10)
                                .style(chip_style(accent_2())),
                            ]
                        )
                        .padding(20)
                        .width(Length::Fill)
                        .style(top_bar_style),

                        Space::new().height(Length::Fixed(14.0)),

                        container(
                            widget::column![
                                widget::row![
                                    help_card(
                                        tr(language, "Chat locally"),
                                        tr(language, "Select one of your installed Ollama models, type a prompt, and press Enter to generate a response."),
                                        accent(),
                                    ),
                                    Space::new().width(Length::Fixed(12.0)),
                                    help_card(
                                        tr(language, "Manage models"),
                                        tr(language, "Use Advanced Settings to install models by name, change the Ollama address, or tune response rendering."),
                                        accent_2(),
                                    ),
                                ],

                                Space::new().height(Length::Fixed(12.0)),

                                widget::row![
                                    help_card(
                                        tr(language, "System prompts"),
                                        tr(language, "System prompts let you switch the assistant's behaviour or personality without rewriting your prompt each time."),
                                        warning(),
                                    ),
                                    Space::new().width(Length::Fixed(12.0)),
                                    help_card(
                                        tr(language, "Chat history"),
                                        tr(language, "When enabled, conversations can be saved locally. You can wipe the current history from Settings."),
                                        danger(),
                                    ),
                                ],

                                Space::new().height(Length::Fixed(16.0)),

                                container(
                                    widget::column![
                                    widget::text(tr(language, "Files and configuration"))
                                            .size(17)
                                            .color(text_main()),
                                        Space::new().height(Length::Fixed(8.0)),
                                        widget::text(
                                            tr(language, "User settings, generated images, and chats are stored in your local application-data folder. Installed assets remain read-only.")
                                        )
                                        .size(14)
                                        .color(text_muted()),
                                    ]
                                )
                                .padding(16)
                                .width(Length::Fill)
                                .style(flat_card_style),
                            ]
                        )
                        .padding(18)
                        .width(Length::Fill)
                        .style(conversation_style),

                        Space::new().height(Length::Fixed(14.0)),

                        widget::row![
                            Space::new().width(Length::Fill),
                            primary_button(tr(language, "Back to chat"), Message::ToggleInfoPopup),
                        ],
                    ]
                )
                .padding(0)
                .width(Length::Fill);

                container(widget::scrollable(content).height(Length::Fill))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::Main => {
                let user_information = self.user_information.clone();
                let bots_list = self.app_state.bots_list.lock().unwrap().clone();
                let copied_text = self.last_copied_text.clone();
                let local_ollamastate = self.app_state.ollama_state.lock().unwrap().clone();
                let active_prompt = self.current_active_prompt();
                let is_processing = active_prompt.is_some();
                let current_web_search_enabled = active_prompt
                    .map(|job| job.web_search_enabled)
                    .unwrap_or(self.web_search_for_chat);
                let web_search_state = active_prompt
                    .map(|job| job.web_search_state.clone())
                    .unwrap_or(WebSearchState::Idle);
                let web_synthesis_thinking = active_prompt
                    .and_then(|job| match &job.web_search_state {
                        WebSearchState::Synthesizing { thinking, .. } => Some(thinking.as_str()),
                        _ => None,
                    })
                    .unwrap_or_default();
                let live_thinking = active_prompt
                    .map(|job| job.thinking_text.as_str())
                    .unwrap_or_default();
                let live_thinking = if live_thinking.is_empty() {
                    web_synthesis_thinking
                } else {
                    live_thinking
                };

                let chat_messages = &self.chat_messages_cache;
                let chat_is_empty = chat_messages.is_empty();

                let selected_model = self.user_information.model.clone();
                let active_model_name = selected_model
                    .clone()
                    .unwrap_or_else(|| tr(language, "No model selected").to_string());
                let current_chat_title = if self.temporary_chat {
                    tr(language, "Temporary chat").to_string()
                } else {
                    self.saved_chats
                        .iter()
                        .find(|chat| chat.id == self.current_chat_id)
                        .map(|chat| chat.title.clone())
                        .or_else(|| {
                            chat_messages.iter().find_map(|message| match message {
                                Correspondence::User { text, .. } => Some(text.clone()),
                                Correspondence::Bot { .. } => None,
                            })
                        })
                        .map(|title| ellipsize_chat_title(&title, 48))
                        .unwrap_or_else(|| tr(language, "New conversation").to_string())
                };

                let prompt_to_send = self.prompt.prompt.clone();
                let prompt = widget::text_editor(&self.prompt.editor)
                    .placeholder(tr(language, "Ask something..."))
                    .padding(14)
                    .size(18)
                    .font(chat_font(user_information.font_family))
                    .height(Length::Fill)
                    .min_height(52)
                    .on_action(Message::EditPrompt)
                    .key_binding(move |key_press| {
                        if matches!(
                            key_press.key.as_ref(),
                            iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                        ) && !key_press.modifiers.shift()
                        {
                            Some(widget::text_editor::Binding::Custom(Message::Prompt(
                                prompt_to_send.clone(),
                            )))
                        } else {
                            widget::text_editor::Binding::from_key_press(key_press)
                        }
                    })
                    .style(text_editor_style);
                let web_toggle: Element<Message> = if is_processing {
                    container(
                        widget::text(if current_web_search_enabled {
                            tr(language, "Web on")
                        } else {
                            tr(language, "Web off")
                        })
                        .size(12)
                        .color(text_muted()),
                    )
                    .padding([7, 9])
                    .style(chip_style(text_muted()))
                    .into()
                } else {
                    mini_button(
                        if current_web_search_enabled {
                            tr(language, "Web on")
                        } else {
                            tr(language, "Web off")
                        },
                        Message::ToggleChatWebSearch,
                    )
                };

                let chat_widgets: Vec<Element<Message>> = chat_messages
                    .iter()
                    .enumerate()
                    .flat_map(|(index, message)| {
                        let parsed_markdown =
                            self.chat_markdown_cache.get(index).map(Vec::as_slice);
                        let cached_thinking = self
                            .chat_thinking_cache
                            .get(index)
                            .map(String::as_str)
                            .unwrap_or_default();
                        let message_model_name = self
                            .chat_model_name_cache
                            .get(index)
                            .cloned()
                            .flatten()
                            .unwrap_or_else(|| active_model_name.clone());
                        let reveal = eased(self.page_reveal * 1.3 - (index.min(10) as f32 * 0.025));
                        let motion = 1.0 - (self.ui_motion * 2.0 - 1.0).abs();

                        vec![
                            message_bubble(
                                index,
                                message,
                                parsed_markdown,
                                cached_thinking,
                                user_information.text_size,
                                user_information.font_family,
                                message_model_name,
                                copied_text.as_ref(),
                                self.expanded_thinking.contains(&index),
                                self.expanded_sources.contains(&index),
                                language,
                                self.code_checking_enabled,
                                self.show_tokens_per_second,
                                &self.markdown_images,
                                reveal,
                                motion,
                            ),
                            Space::new().height(Length::Fixed(10.0)).into(),
                        ]
                    })
                    .collect();

                let online = local_ollamastate.to_lowercase() != "offline";
                let status_color = if online { success() } else { danger() };
                let pulse = 1.0 - (self.ui_motion * 2.0 - 1.0).abs();
                let visible_debug = self.current_debug_message().clone();
                let debug_color = if visible_debug.is_error {
                    danger()
                } else {
                    success()
                };

                let model_selector: Element<Message> = if bots_list.is_empty() {
                    container(
                        widget::text(tr(language, "No models installed"))
                            .size(13)
                            .color(text_muted()),
                    )
                    .padding(10)
                    .style(chip_style(danger()))
                    .into()
                } else {
                    widget::pick_list(
                        bots_list.clone(),
                        selected_model.clone(),
                        Message::ModelChange,
                    )
                    .padding([12, 14])
                    .text_size(14)
                    .style(pick_list_style)
                    .menu_style(pick_list_menu_style)
                    .width(Length::Fill)
                    .into()
                };

                let thinking_selector: Element<Message> = widget::pick_list(
                    ThinkingChoice::from_levels(&self.user_information.thinking_levels, language),
                    Some(ThinkingChoice {
                        level: self.user_information.thinking_level,
                        language,
                    }),
                    |choice| Message::ThinkingLevelChange(choice.level),
                )
                .placeholder(tr(language, "Thinking"))
                .padding([12, 14])
                .text_size(14)
                .style(pick_list_style)
                .menu_style(pick_list_menu_style)
                .width(Length::Fixed(132.0))
                .into();

                let live_response: Element<Message> = if let Some(active_prompt) = active_prompt {
                    let response_model_name = active_prompt.model_name.clone();
                    let elapsed_seconds = active_prompt.started_at.elapsed().as_secs();
                    let activity = match &web_search_state {
                        WebSearchState::Searching { .. } => tr(language, "Searching"),
                        WebSearchState::Results { .. } => tr(language, "Reviewing results"),
                        WebSearchState::Fetching { .. } => tr(language, "Reading website"),
                        WebSearchState::Failed { .. } => tr(language, "Web search"),
                        WebSearchState::Idle
                        | WebSearchState::Synthesizing { .. }
                        | WebSearchState::Completed => {
                            if language == Language::Spanish {
                                "Pensando"
                            } else {
                                "Thinking"
                            }
                        }
                    };
                    let activity_dots = ".".repeat(((self.ui_motion * 4.0) as usize % 3) + 1);
                    let label = ellipsize_chat_title(
                        &format!(
                            "{response_model_name} · {activity}{activity_dots} ({elapsed_seconds}s)"
                        ),
                        64,
                    );
                    let web_search_activity_visible = !matches!(
                        &web_search_state,
                        WebSearchState::Idle | WebSearchState::Completed
                    );
                    let search_activity = web_search_activity(web_search_state.clone(), language);
                    let search_gap: Element<Message> = if web_search_activity_visible {
                        Space::new().height(Length::Fixed(8.0)).into()
                    } else {
                        widget::column![].into()
                    };

                    let live_reasoning: Element<Message> = if live_thinking.is_empty() {
                        widget::column![].into()
                    } else {
                        let expanded = self.expanded_thinking.contains(&usize::MAX);
                        let details: Element<Message> = if expanded {
                            container(
                                widget::text(live_thinking)
                                    .size(user_information.text_size - 1.0)
                                    .font(chat_font(user_information.font_family))
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
                            mini_button(
                                tr(
                                    language,
                                    if expanded {
                                        "▾ Hide thinking"
                                    } else {
                                        "▸ Show thinking"
                                    }
                                ),
                                Message::ToggleThinking(usize::MAX),
                            ),
                            details,
                            Space::new().height(Length::Fixed(7.0)),
                        ]
                        .into()
                    };

                    widget::row![
                        container(widget::text("✦").size(17).color(Color::WHITE))
                            .padding([8, 11])
                            .style(assistant_mark_style(pulse, true)),
                        Space::new().width(Length::Fixed(10.0)),
                        container(widget::column![
                            widget::row![
                                container(
                                    widget::text(label)
                                        .size(12)
                                        .color(accent_2())
                                        .wrapping(Wrapping::None)
                                )
                                .width(Length::Fill)
                                .clip(true),
                                Space::new().width(Length::Fixed(10.0)),
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(96.0))
                                    .girth(Length::Fixed(5.0)),
                            ],
                            Space::new().height(Length::Fixed(8.0)),
                            search_activity,
                            search_gap,
                            live_reasoning,
                            markdown_with_code_copy(
                                &active_prompt.parsed_markdown,
                                user_information.text_size,
                                user_information.font_family,
                                copied_text.as_ref(),
                                language,
                                self.code_checking_enabled,
                                &self.markdown_images,
                                self.ui_motion,
                            ),
                        ])
                        .padding(14)
                        .width(Length::Fill)
                        .max_width(920)
                        .style(bot_bubble_style(1.0)),
                        Space::new().width(Length::Fixed(24.0)),
                    ]
                    .into()
                } else if chat_is_empty {
                    let (suggestion_labels, suggestion_prompts) = if language == Language::Spanish {
                        (
                            [
                                "Explicar un tema complejo",
                                "Planificar un proyecto",
                                "Escribir o mejorar código",
                                "Generar ideas",
                            ],
                            [
                                "Explica un tema complejo de forma sencilla, con una analogía y un ejemplo práctico.",
                                "Ayúdame a convertir una idea en un plan claro con próximos pasos y posibles riesgos.",
                                "Ayúdame a escribir o mejorar código. Primero pregúntame por el lenguaje y el objetivo.",
                                "Genera varias ideas creativas, compáralas brevemente y recomienda las tres mejores.",
                            ],
                        )
                    } else {
                        (
                            [
                                "Explain a complex topic",
                                "Plan a project",
                                "Write or improve code",
                                "Brainstorm ideas",
                            ],
                            [
                                "Explain a complex topic simply, using an analogy and one practical example.",
                                "Help me turn an idea into a clear plan with next steps and likely risks.",
                                "Help me write or improve some code. Start by asking for the language and goal.",
                                "Brainstorm several creative ideas, compare them briefly, and recommend the best three.",
                            ],
                        )
                    };
                    widget::row![
                        Space::new().width(Length::Fill),
                        container(
                            widget::column![
                                container(widget::text("✦").size(28).color(Color::WHITE))
                                    .padding([
                                        (13.0 + pulse * 0.75) as u16,
                                        (17.0 + pulse * 0.75) as u16,
                                    ])
                                    .style(assistant_mark_style(pulse, true)),
                                Space::new().height(Length::Fixed(18.0)),
                                widget::text(tr(language, "What can I help you make?"))
                                    .size(28)
                                    .color(text_main())
                                    .align_x(Horizontal::Center),
                                Space::new().height(Length::Fixed(8.0)),
                                widget::text(tr(
                                    language,
                                    "Choose a starting point below, or write your own message."
                                ))
                                .size(14)
                                .color(text_muted())
                                .align_x(Horizontal::Center),
                                Space::new().height(Length::Fixed(22.0)),
                                suggestion_grid(suggestion_labels, suggestion_prompts),
                            ]
                            .align_x(Horizontal::Center)
                            .width(Length::Fill)
                        )
                        .padding([42, 24])
                        .width(Length::Fill)
                        .max_width(760),
                        Space::new().width(Length::Fill),
                    ]
                    .into()
                } else {
                    widget::column![].into()
                };

                let offline_hint: Element<Message> = if !online {
                    container(widget::row![
                        widget::column![
                            widget::text(tr(language, "Ollama was not detected."))
                                .size(14)
                                .color(text_main()),
                            Space::new().height(Length::Fixed(3.0)),
                            widget::text(tr(
                                language,
                                "Install Ollama or check your connection settings."
                            ))
                            .size(12)
                            .color(text_muted()),
                        ]
                        .width(Length::Fill),
                        secondary_button(
                            tr(language, "Install Ollama"),
                            Message::InstallationPrompt
                        ),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                } else {
                    widget::column![].into()
                };

                let missing_bots_hint: Element<Message> = if bots_list.is_empty() {
                    container(widget::row![
                        widget::column![
                            widget::text(tr(language, "No models were detected."))
                                .size(14)
                                .color(text_main()),
                            Space::new().height(Length::Fixed(3.0)),
                            widget::text(tr(language, "Install a model before sending prompts."))
                                .size(12)
                                .color(text_muted()),
                        ]
                        .width(Length::Fill),
                        secondary_button(tr(language, "Find models"), Message::ListPrompt),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                } else {
                    widget::column![].into()
                };

                let sidebar_width =
                    82.0 + self.sidebar_animation * (self.ui_layout.sidebar_width - 82.0);
                let show_sidebar_details = self.sidebar_animation > 0.52;
                let chat_sidebar: Element<Message> = if show_sidebar_details {
                    let mut entries: Vec<Element<Message>> = vec![
                        primary_button(tr(language, "＋ New chat"), Message::NewChat),
                        Space::new().height(Length::Fixed(6.0)).into(),
                        secondary_button(
                            if self.temporary_chat {
                                tr(language, "Leave temporary chat")
                            } else {
                                tr(language, "Temporary chat")
                            },
                            Message::ToggleTemporaryChat,
                        ),
                        Space::new().height(Length::Fixed(14.0)).into(),
                    ];
                    let has_temporary_chats = self.temporary_chat
                        || !self.temporary_chats.is_empty()
                        || self.active_prompts.values().any(|job| job.temporary);
                    if has_temporary_chats {
                        entries.push(
                            widget::text(tr(language, "Temporary chats"))
                                .size(13)
                                .color(text_muted())
                                .into(),
                        );
                    }
                    let mut temporary_jobs = self
                        .active_prompts
                        .iter()
                        .filter(|(_, job)| {
                            job.temporary && job.profile_id == self.active_profile_id
                        })
                        .collect::<Vec<_>>();
                    temporary_jobs.sort_by_key(|(_, job)| job.started_at);
                    for (chat_id, job) in temporary_jobs {
                        let title = job
                            .chat_history
                            .lock()
                            .ok()
                            .and_then(|chat| {
                                chat.messages.iter().find_map(|message| match message {
                                    Correspondence::User { text, .. } => Some(text.clone()),
                                    Correspondence::Bot { .. } => None,
                                })
                            })
                            .unwrap_or_else(|| tr(language, "Temporary chat").to_string());
                        let title = format!("T · {}", ellipsize_chat_title(&title, 16));
                        entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(title).size(13).wrapping(Wrapping::None)
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(42.0))
                                    .girth(Length::Fixed(4.0)),
                            ])
                            .padding(4)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    let mut temporary_sessions = self
                        .temporary_chats
                        .iter()
                        .filter(|(_, session)| session.profile_id == self.active_profile_id)
                        .collect::<Vec<_>>();
                    temporary_sessions.sort_by_key(|(chat_id, _)| *chat_id);
                    for (chat_id, session) in temporary_sessions {
                        let title = session
                            .chat_history
                            .lock()
                            .ok()
                            .and_then(|chat| {
                                chat.messages.iter().find_map(|message| match message {
                                    Correspondence::User { text, .. } => Some(text.clone()),
                                    Correspondence::Bot { .. } => None,
                                })
                            })
                            .unwrap_or_else(|| tr(language, "Temporary chat").to_string());
                        let title = format!("T · {}", ellipsize_chat_title(&title, 16));
                        entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(title).size(13).wrapping(Wrapping::None)
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                mini_button("×", Message::DeleteTemporaryChat(chat_id.clone())),
                            ])
                            .padding(4)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    if has_temporary_chats {
                        entries.push(Space::new().height(Length::Fixed(8.0)).into());
                    }
                    entries.push(
                        widget::text(tr(language, "Saved chats"))
                            .size(13)
                            .color(text_muted())
                            .into(),
                    );
                    for saved in self
                        .saved_chats
                        .iter()
                        .filter(|chat| crate::chat_profile_id(chat) == self.active_profile_id)
                    {
                        let selected = saved.id == self.current_chat_id;
                        let title = ellipsize_chat_title(&saved.title, 20);
                        let working = self.active_prompts.contains_key(&saved.id);
                        let working_progress: Element<Message> = if working {
                            widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                .length(Length::Fixed(42.0))
                                .girth(Length::Fixed(4.0))
                                .into()
                        } else {
                            widget::column![].into()
                        };
                        entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(title).size(13).wrapping(Wrapping::None)
                                )
                                .on_press(Message::OpenChat(saved.id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                working_progress,
                                mini_button(
                                    tr(language, if saved.pinned { "Unpin" } else { "Pin" }),
                                    Message::ToggleChatPin(saved.id.clone()),
                                ),
                                mini_button("×", Message::DeleteChat(saved.id.clone())),
                            ])
                            .padding(4)
                            .width(Length::Fill)
                            .style(chat_entry_style(selected))
                            .into(),
                        );
                    }
                    container(widget::column![
                        widget::row![
                            container(
                                widget::image(self.brand_icon.clone())
                                    .width(Length::Fixed(25.0))
                                    .height(Length::Fixed(25.0))
                                    .content_fit(iced::ContentFit::Contain)
                                    .border_radius(8.0)
                            )
                            .padding(4)
                            .center_x(Length::Fill)
                            .center_y(Length::Fill)
                            .width(Length::Fixed(35.0))
                            .height(Length::Fixed(35.0))
                            .style(status_brand_style(status_color)),
                            Space::new().width(Length::Fixed(9.0)),
                            widget::column![
                                widget::text("LOCORYN").size(10).color(accent_2()),
                                widget::text(tr(language, "Chats"))
                                    .size(17)
                                    .color(text_main()),
                            ],
                            Space::new().width(Length::Fill),
                            mini_button("«", Message::ToggleChatMenu),
                        ],
                        Space::new().height(Length::Fixed(18.0)),
                        widget::scrollable(
                            widget::Column::with_children(entries).spacing(iced::Pixels(6.0))
                        ),
                        // Reserve room for the profile switcher overlaid in
                        // the bottom-left corner.
                        Space::new().height(Length::Fixed(46.0)),
                    ])
                    .padding(12)
                    .width(Length::Fixed(sidebar_width))
                    .height(Length::Fill)
                    .style(sidebar_style)
                    .into()
                } else {
                    let mut compact_entries: Vec<Element<Message>> = vec![
                        mini_button("☰", Message::ToggleChatMenu),
                        mini_button("＋", Message::NewChat),
                        mini_button(
                            if self.temporary_chat {
                                "◌ ✓"
                            } else {
                                "◌"
                            },
                            Message::ToggleTemporaryChat,
                        ),
                        Space::new().height(Length::Fixed(4.0)).into(),
                    ];
                    let mut temporary_jobs = self
                        .active_prompts
                        .iter()
                        .filter(|(_, job)| {
                            job.temporary && job.profile_id == self.active_profile_id
                        })
                        .collect::<Vec<_>>();
                    temporary_jobs.sort_by_key(|(_, job)| job.started_at);
                    for (chat_id, _) in temporary_jobs {
                        compact_entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text("T · work").size(11).wrapping(Wrapping::None),
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .padding([6, 4])
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(14.0))
                                    .girth(Length::Fixed(3.0)),
                            ])
                            .padding(2)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    let mut temporary_sessions = self
                        .temporary_chats
                        .iter()
                        .filter(|(_, session)| session.profile_id == self.active_profile_id)
                        .map(|(chat_id, _)| chat_id)
                        .collect::<Vec<_>>();
                    temporary_sessions.sort();
                    for chat_id in temporary_sessions {
                        compact_entries.push(
                            container(
                                widget::button(
                                    widget::text("T · done").size(11).wrapping(Wrapping::None),
                                )
                                .on_press(Message::OpenChat(chat_id.clone()))
                                .padding([6, 4])
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                            )
                            .padding(2)
                            .width(Length::Fill)
                            .style(chat_entry_style(chat_id == &self.current_chat_id))
                            .into(),
                        );
                    }
                    for saved in self
                        .saved_chats
                        .iter()
                        .filter(|chat| crate::chat_profile_id(chat) == self.active_profile_id)
                    {
                        let short_title = ellipsize_chat_title(&saved.title, 8);
                        let working = self.active_prompts.contains_key(&saved.id);
                        let working_progress: Element<Message> = if working {
                            widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                .length(Length::Fixed(14.0))
                                .girth(Length::Fixed(3.0))
                                .into()
                        } else {
                            widget::column![].into()
                        };
                        compact_entries.push(
                            container(widget::row![
                                widget::button(
                                    widget::text(short_title).size(11).wrapping(Wrapping::None),
                                )
                                .on_press(Message::OpenChat(saved.id.clone()))
                                .padding([6, 4])
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                working_progress,
                            ])
                            .padding(2)
                            .width(Length::Fill)
                            .style(chat_entry_style(saved.id == self.current_chat_id))
                            .into(),
                        );
                    }
                    container(widget::column![
                        widget::scrollable(
                            widget::Column::with_children(compact_entries)
                                .spacing(iced::Pixels(5.0)),
                        ),
                        // Reserve room for the profile switcher overlaid in
                        // the bottom-left corner.
                        Space::new().height(Length::Fixed(46.0)),
                    ])
                    .padding(8)
                    .width(Length::Fixed(sidebar_width))
                    .height(Length::Fill)
                    .style(sidebar_style)
                    .into()
                };

                let active_profile_name = self.active_profile_name();
                let profile_chip_label: Element<Message> = if show_sidebar_details {
                    widget::row![
                        widget::text("👤").size(13),
                        widget::text(ellipsize_chat_title(&active_profile_name, 18))
                            .size(14)
                            .wrapping(Wrapping::None),
                        widget::text(if self.profile_menu_open { "▾" } else { "▴" })
                            .size(11)
                            .color(text_muted()),
                    ]
                    .spacing(7)
                    .into()
                } else {
                    widget::text("👤").size(14).into()
                };
                let profile_chip: Element<Message> = widget::button(profile_chip_label)
                    .on_press(Message::ToggleProfileMenu)
                    .padding(if show_sidebar_details {
                        [8, 13]
                    } else {
                        [8, 9]
                    })
                    .style(profile_chip_style(self.profile_menu_open))
                    .into();
                let profile_switcher: Element<Message> = if self.profile_menu_open {
                    let mut rows: Vec<Element<Message>> = Vec::new();
                    for profile in &self.profiles {
                        let is_active = profile.id == self.active_profile_id;
                        let is_editing = self
                            .editing_profile_id
                            .as_deref()
                            .is_some_and(|editing| editing == profile.id);
                        let active_marker: Element<Message> = if is_active {
                            widget::text("✓").size(13).color(accent_2()).into()
                        } else {
                            widget::column![].into()
                        };
                        rows.push(
                            widget::row![
                                widget::button(
                                    widget::text(ellipsize_chat_title(&profile.name, 24))
                                        .size(14)
                                        .wrapping(Wrapping::None)
                                        .color(if is_active { accent_2() } else { text_main() }),
                                )
                                .on_press(Message::SelectProfile(profile.id.clone()))
                                .style(chat_title_button_style)
                                .clip(true)
                                .width(Length::Fill),
                                active_marker,
                                mini_button(
                                    if is_editing { "▾" } else { "✎" },
                                    Message::StartEditProfile(profile.id.clone())
                                ),
                                mini_button("×", Message::DeleteProfile(profile.id.clone())),
                            ]
                            .spacing(5)
                            .into(),
                        );
                    }
                    let rows_height = (rows.len().min(5) * 40) as f32;
                    let edit_panel: Element<Message> = if self.editing_profile_id.is_some()
                        && self.profiles.iter().any(|profile| {
                            Some(profile.id.as_str()) == self.editing_profile_id.as_deref()
                        }) {
                        container(
                            widget::column![
                                widget::text(tr(language, "EDIT PROFILE"))
                                    .size(10)
                                    .color(accent_2()),
                                iced::widget::TextInput::<Message>::new(
                                    tr(language, "Profile name (only you see this)"),
                                    &self.profile_edit_name,
                                )
                                .on_input(Message::ProfileEditNameChanged)
                                .padding(9)
                                .width(Length::Fill)
                                .style(text_input_style),
                                iced::widget::TextInput::<Message>::new(
                                    tr(language, "User name (shared with the model)"),
                                    &self.profile_edit_user_name,
                                )
                                .on_input(Message::ProfileEditUserNameChanged)
                                .padding(9)
                                .width(Length::Fill)
                                .style(text_input_style),
                                iced::widget::TextInput::<Message>::new(
                                    tr(language, "Extra instructions for the model"),
                                    &self.profile_edit_instructions,
                                )
                                .on_input(Message::ProfileEditInstructionsChanged)
                                .on_submit(Message::ConfirmProfileEdits)
                                .padding(9)
                                .width(Length::Fill)
                                .style(text_input_style),
                                widget::row![
                                    Space::new().width(Length::Fill),
                                    mini_button(tr(language, "Cancel"), Message::CancelEditProfile),
                                    mini_button(tr(language, "Save"), Message::ConfirmProfileEdits),
                                ]
                                .spacing(5),
                            ]
                            .spacing(7),
                        )
                        .padding(10)
                        .width(Length::Fill)
                        .style(flat_card_style)
                        .into()
                    } else {
                        widget::column![].into()
                    };
                    let popup: Element<Message> = container(
                        widget::column![
                            widget::text(tr(language, "PROFILES"))
                                .size(11)
                                .color(accent_2()),
                            widget::scrollable(
                                widget::Column::with_children(rows).spacing(iced::Pixels(6.0))
                            )
                            .height(Length::Fixed(rows_height)),
                            edit_panel,
                            widget::row![
                                iced::widget::TextInput::<Message>::new(
                                    tr(language, "New profile name"),
                                    &self.profile_name_input,
                                )
                                .on_input(Message::ProfileNameInputChanged)
                                .on_submit(Message::CreateProfile)
                                .padding(9)
                                .width(Length::Fill)
                                .style(text_input_style),
                                mini_button("＋", Message::CreateProfile),
                            ]
                            .spacing(5),
                        ]
                        .spacing(9),
                    )
                    .padding(12)
                    .width(Length::Fixed(290.0))
                    .style(profile_popup_style)
                    .into();
                    widget::column![popup, Space::new().height(Length::Fixed(6.0)), profile_chip,]
                        .align_x(iced::Alignment::Start)
                        .into()
                } else {
                    profile_chip
                };
                // The switcher floats in the bottom-left corner, over the
                // chat list, so it stays reachable on every page state.
                let chat_sidebar: Element<Message> = widget::stack![
                    chat_sidebar,
                    container(profile_switcher)
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .align_x(iced::Alignment::Start)
                        .align_y(iced::Alignment::End)
                        .padding(10),
                ]
                .into();

                let copy_response_action: Element<Message> = if chat_is_empty {
                    widget::column![].into()
                } else {
                    mini_button(tr(language, "Copy response"), Message::CopyLatestResponse)
                };
                let composer_active =
                    !self.prompt.prompt.trim().is_empty() || !self.pending_images.is_empty();
                let prompt_input: Element<Message> = container(prompt)
                    .padding(3)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(input_shell_style(composer_active, pulse))
                    .into();
                let composer_input: Element<Message> = if self.pending_images.is_empty() {
                    prompt_input
                } else {
                    widget::row![
                        composer_image_previews(&self.pending_images),
                        Space::new().width(Length::Fixed(8.0)),
                        prompt_input,
                    ]
                    .align_y(iced::Alignment::Center)
                    .height(Length::Fill)
                    .into()
                };

                let content = widget::column![
                    Space::new().height(Length::Fixed((1.0 - eased(self.page_reveal)) * 4.0)),
                    container(widget::column![
                        widget::row![
                            widget::column![
                                widget::text(tr(language, "LOCAL AI WORKSPACE"))
                                    .size(10)
                                    .color(accent_2()),
                                Space::new().height(Length::Fixed(3.0)),
                                widget::text(current_chat_title).size(20).color(text_main()),
                            ],
                            Space::new().width(Length::Fill),
                            toolbar_button("▣", tr(language, "Images"), Message::ToggleImages),
                            Space::new().width(Length::Fixed(6.0)),
                            toolbar_button("⚙", tr(language, "Settings"), Message::ToggleSettings),
                        ],
                        Space::new().height(Length::Fixed(16.0)),
                        widget::row![
                            widget::column![
                                widget::text(tr(language, "MODEL"))
                                    .size(10)
                                    .color(text_faint()),
                                Space::new().height(Length::Fixed(5.0)),
                                container(model_selector).width(Length::Fill),
                            ]
                            .width(Length::FillPortion(5)),
                            Space::new().width(Length::Fixed(10.0)),
                            widget::column![
                                widget::text(tr(language, "SYSTEM PROMPT"))
                                    .size(10)
                                    .color(text_faint()),
                                Space::new().height(Length::Fixed(5.0)),
                                widget::pick_list(
                                    self.system_prompt
                                        .system_prompts_as_vec
                                        .lock()
                                        .unwrap()
                                        .clone(),
                                    self.system_prompt.system_prompt.clone(),
                                    Message::SystemPromptChange,
                                )
                                .placeholder(tr(language, "System prompt"))
                                .padding([12, 14])
                                .text_size(14)
                                .style(pick_list_style)
                                .menu_style(pick_list_menu_style)
                                .width(Length::Fill),
                            ]
                            .width(Length::FillPortion(3)),
                            Space::new().width(Length::Fixed(10.0)),
                            widget::column![
                                widget::text(tr(language, "REASONING"))
                                    .size(10)
                                    .color(text_faint()),
                                Space::new().height(Length::Fixed(5.0)),
                                thinking_selector,
                            ]
                            .width(Length::Fixed(150.0)),
                        ],
                    ])
                    .padding(16)
                    .width(Length::Fill)
                    .style(top_bar_style),
                    Space::new().height(Length::Fixed(10.0)),
                    container(
                        widget::scrollable(
                            widget::column![
                                widget::Column::with_children(chat_widgets)
                                    .spacing(iced::Pixels(3.0)),
                                live_response,
                                Space::new().height(Length::Fixed(18.0)),
                            ]
                            .spacing(iced::Pixels(6.0))
                        )
                        .height(Length::Fill)
                        .anchor_bottom()
                    )
                    .padding([20, 18])
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(conversation_style),
                    composer_resize_handle(),
                    container(
                        widget::column![
                            composer_input,
                            Space::new().height(Length::Fixed(9.0)),
                            widget::row![
                                mini_button(tr(language, "＋ Attach"), Message::PickImage),
                                Space::new().width(Length::Fixed(5.0)),
                                mini_button(tr(language, "Paste"), Message::PasteImage),
                                Space::new().width(Length::Fixed(5.0)),
                                web_toggle,
                                Space::new().width(Length::Fill),
                                widget::text(tr(language, "Enter to send"))
                                    .size(11)
                                    .color(text_faint()),
                                Space::new().width(Length::Fixed(8.0)),
                                copy_response_action,
                                Space::new().width(Length::Fixed(6.0)),
                                if is_processing {
                                    danger_button(tr(language, "■ Stop"), Message::StopResponse)
                                } else {
                                    send_button(
                                        tr(language, "Send"),
                                        (!self.prompt.prompt.trim().is_empty())
                                            .then(|| Message::Prompt(self.prompt.prompt.clone())),
                                    )
                                },
                            ],
                            offline_hint,
                            missing_bots_hint,
                            widget::row![
                                widget::text(visible_debug.message)
                                    .size(13)
                                    .color(debug_color),
                            ],
                        ]
                        .height(Length::Fill)
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .height(Length::Fixed(self.ui_layout.composer_height))
                    .style(composer_style(composer_active, pulse)),
                ]
                .spacing(iced::Pixels(0.0));

                let sidebar_handle: Element<Message> =
                    if self.chat_menu_open && self.sidebar_animation >= 0.998 {
                        sidebar_resize_handle()
                    } else {
                        Space::new().width(Length::Fixed(10.0)).into()
                    };
                container(widget::row![chat_sidebar, sidebar_handle, content])
                    .padding(10)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::Images => {
                let bots_list = self.app_state.bots_list.lock().unwrap().clone();
                let selected_model = self.user_information.model.clone();
                let active_prompt = self.current_active_prompt();
                let vision_is_live = active_prompt.is_some_and(|job| job.had_image);
                let completed_vision = self.vision_responses.get(&self.current_chat_id);
                let visible_debug = self.current_debug_message().clone();
                let pulse = 1.0 - (self.ui_motion * 2.0 - 1.0).abs();
                let model_selector =
                    widget::pick_list(bots_list, selected_model, Message::ModelChange)
                        .padding([12, 14])
                        .text_size(14)
                        .style(pick_list_style)
                        .menu_style(pick_list_menu_style)
                        .width(Length::Fill);

                let attachment: Element<Message> = if !self.pending_images.is_empty() {
                    image_previews(&self.pending_images, true, language)
                } else {
                    container(widget::column![
                        widget::text(tr(language, "Add an image for vision"))
                            .size(17)
                            .color(text_main()),
                        Space::new().height(Length::Fixed(5.0)),
                        widget::text(tr(
                            language,
                            "Paste from the clipboard or choose a local image."
                        ))
                        .size(12)
                        .color(text_muted()),
                        Space::new().height(Length::Fixed(14.0)),
                        widget::row![
                            secondary_button(tr(language, "Choose image"), Message::PickImage),
                            Space::new().width(Length::Fixed(8.0)),
                            secondary_button(tr(language, "Paste image"), Message::PasteImage),
                        ],
                    ])
                    .padding(22)
                    .width(Length::Fill)
                    .style(flat_card_style)
                    .into()
                };

                let capability_status: Element<Message> = match self
                    .user_information
                    .vision_supported
                {
                    Some(true) => {
                        container(widget::text(tr(language, "This model can inspect images.")))
                            .padding(9)
                            .style(chip_style(success()))
                            .into()
                    }
                    Some(false) => container(widget::text(tr(
                        language,
                        "This model does not support image input.",
                    )))
                    .padding(9)
                    .style(chip_style(danger()))
                    .into(),
                    None => container(widget::text(tr(language, "Checking image capabilities…")))
                        .padding(9)
                        .style(chip_style(brighten(warning(), pulse * 0.025)))
                        .into(),
                };

                let vision_prompt_to_send = self.prompt.prompt.clone();
                let vision_prompt = widget::text_editor(&self.prompt.editor)
                    .placeholder(tr(
                        language,
                        "Describe an image, or ask a question about the attached image…",
                    ))
                    .padding(14)
                    .size(16)
                    .font(chat_font(self.user_information.font_family))
                    .min_height(72)
                    .max_height(180)
                    .on_action(Message::EditPrompt)
                    .key_binding(move |key_press| {
                        if matches!(
                            key_press.key.as_ref(),
                            iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                        ) && !key_press.modifiers.shift()
                        {
                            Some(widget::text_editor::Binding::Custom(Message::Prompt(
                                vision_prompt_to_send.clone(),
                            )))
                        } else {
                            widget::text_editor::Binding::from_key_press(key_press)
                        }
                    })
                    .style(text_editor_style);

                let vision_action: Element<Message> = if vision_is_live {
                    primary_button(tr(language, "Stop"), Message::StopResponse)
                } else if self.user_information.vision_supported != Some(false)
                    && !self.pending_images.is_empty()
                {
                    primary_button(
                        tr(language, "Ask about image"),
                        Message::Prompt(self.prompt.prompt.clone()),
                    )
                } else {
                    widget::column![].into()
                };

                let vision_response: Element<Message> = if vision_is_live
                    || completed_vision.is_some()
                {
                    let markdown = active_prompt
                        .filter(|job| job.had_image)
                        .map(|job| job.parsed_markdown.as_slice())
                        .or_else(|| completed_vision.map(|response| response.markdown.as_slice()))
                        .unwrap_or_default();
                    container(widget::column![
                        widget::row![
                            widget::text(tr(
                                language,
                                if vision_is_live {
                                    "Vision model is responding…"
                                } else {
                                    "Vision response"
                                }
                            ))
                            .size(12)
                            .color(accent_2()),
                            Space::new().width(Length::Fill),
                            if vision_is_live {
                                widget::progress_bar(0.0..=1.0, self.prompt_progress())
                                    .length(Length::Fixed(96.0))
                                    .girth(Length::Fixed(5.0))
                            } else {
                                widget::progress_bar(0.0..=1.0, 0.0)
                                    .length(Length::Fixed(0.0))
                                    .girth(Length::Fixed(0.0))
                            },
                        ],
                        Space::new().height(Length::Fixed(8.0)),
                        markdown_with_code_copy(
                            markdown,
                            self.user_information.text_size,
                            self.user_information.font_family,
                            self.last_copied_text.as_ref(),
                            language,
                            self.code_checking_enabled,
                            &self.markdown_images,
                            self.ui_motion,
                        ),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(bot_bubble_style(eased(self.page_reveal)))
                    .into()
                } else {
                    widget::column![].into()
                };

                let generated_cards: Vec<Element<Message>> = self
                    .generated_images
                    .iter()
                    .rev()
                    .map(|path| {
                        container(widget::column![
                            widget::image(iced::widget::image::Handle::from_path(path))
                                .height(Length::Fixed(280.0))
                                .width(Length::Fill)
                                .content_fit(iced::ContentFit::Contain)
                                .border_radius(12.0)
                                .opacity(eased(self.page_reveal))
                                .scale(0.99 + eased(self.page_reveal) * 0.01),
                            Space::new().height(Length::Fixed(8.0)),
                            widget::row![
                                widget::text(path.clone())
                                    .size(11)
                                    .color(text_muted())
                                    .wrapping(Wrapping::WordOrGlyph)
                                    .width(Length::Fill),
                                mini_button(
                                    tr(language, "Copy image"),
                                    Message::CopyImage(path.clone())
                                ),
                            ],
                        ])
                        .padding(12)
                        .width(Length::Fill)
                        .style(flat_card_style)
                        .into()
                    })
                    .collect();

                let generation_progress: Element<Message> = if self.is_generating_image {
                    widget::progress_bar(0.0..=1.0, 0.08 + pulse * 0.84)
                        .girth(Length::Fixed(4.0))
                        .into()
                } else {
                    Space::new().height(Length::Fixed(0.0)).into()
                };

                let generation_panel: Element<Message> = if self
                    .user_information
                    .image_generation_supported
                    == Some(true)
                {
                    let generation_prompt = widget::text_editor(&self.prompt.editor)
                        .placeholder(tr(language, "Describe the image you want to generate…"))
                        .padding(14)
                        .size(16)
                        .font(chat_font(self.user_information.font_family))
                        .min_height(72)
                        .max_height(180)
                        .on_action(Message::EditPrompt)
                        .key_binding(|key_press| {
                            if matches!(
                                key_press.key.as_ref(),
                                iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter)
                            ) && !key_press.modifiers.shift()
                            {
                                Some(widget::text_editor::Binding::Custom(Message::GenerateImage))
                            } else {
                                widget::text_editor::Binding::from_key_press(key_press)
                            }
                        })
                        .style(text_editor_style);
                    container(widget::column![
                            setting_label(
                                tr(language, "Experimental image generation"),
                                tr(language, "Ollama reports that this model can generate images. Output is requested through /v1/images/generations at 1024 × 1024.")
                            ),
                            Space::new().height(Length::Fixed(12.0)),
                            generation_prompt,
                            Space::new().height(Length::Fixed(10.0)),
                            primary_button(
                                tr(
                                    language,
                                    if self.is_generating_image {
                                        "Generating…"
                                    } else {
                                        "Generate image"
                                    }
                                ),
                                Message::GenerateImage
                            ),
                            generation_progress,
                        ])
                        .padding(18)
                        .width(Length::Fill)
                        .style(conversation_style)
                        .into()
                } else {
                    widget::column![].into()
                };

                let generated_gallery: Element<Message> = if generated_cards.is_empty() {
                    widget::column![].into()
                } else {
                    container(widget::column![
                        widget::text(tr(language, "Generated images"))
                            .size(18)
                            .color(text_main()),
                        Space::new().height(Length::Fixed(10.0)),
                        widget::Column::with_children(generated_cards).spacing(iced::Pixels(12.0)),
                    ])
                    .padding(14)
                    .width(Length::Fill)
                    .style(conversation_style)
                    .into()
                };

                let content = widget::column![
                    Space::new().height(Length::Fixed(
                        (1.0 - eased(self.page_reveal)) * 4.0
                    )),
                    container(widget::row![
                        section_title(
                            tr(language, "Images"),
                            tr(language, "Analyze images with a vision model. Experimental image generation appears only for models that report support.")
                        ),
                        Space::new().width(Length::Fill),
                        secondary_button(tr(language, "Back to chat"), Message::ToggleImages),
                    ]).padding(18).width(Length::Fill).style(top_bar_style),
                    Space::new().height(Length::Fixed(14.0)),
                    container(widget::column![
                        setting_label(
                            tr(language, "Vision analysis"),
                            tr(language, "Attach an image and ask a vision-capable model to describe, classify, read, or reason about it.")
                        ),
                        Space::new().height(Length::Fixed(12.0)),
                        widget::text(tr(language, "Model"))
                            .size(12)
                            .color(text_faint()),
                        Space::new().height(Length::Fixed(5.0)),
                        model_selector,
                        Space::new().height(Length::Fixed(10.0)),
                        capability_status,
                        Space::new().height(Length::Fixed(14.0)),
                        attachment,
                        Space::new().height(Length::Fixed(12.0)),
                        vision_prompt,
                        Space::new().height(Length::Fixed(10.0)),
                        vision_action,
                    ]).padding(18).width(Length::Fill).style(conversation_style),
                    Space::new().height(Length::Fixed(14.0)),
                    vision_response,
                    Space::new().height(Length::Fixed(14.0)),
                    generation_panel,
                    Space::new().height(Length::Fixed(14.0)),
                    generated_gallery,
                    widget::text(visible_debug.message)
                        .size(13)
                        .color(if visible_debug.is_error { danger() } else { success() }),
                ];

                container(widget::scrollable(content))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::Settings => {
                let user_information = self.user_information.clone();
                let web_api_key = self
                    .web_search_settings
                    .selected_api_key()
                    .unwrap_or_default()
                    .to_string();
                let bots_list = self.app_state.bots_list.lock().unwrap().clone();
                let prompts_list = self
                    .system_prompt
                    .system_prompts_as_vec
                    .lock()
                    .unwrap()
                    .clone();

                let debug_color = if self.debug_message.clone().is_error {
                    danger()
                } else {
                    success()
                };
                let (update_status, update_action): (String, Element<Message>) =
                    match &self.app_update_state {
                        AppUpdateState::Idle => (
                            format!("Current version: v{}", crate::APP_VERSION),
                            secondary_button(tr(language, "Check now"), Message::CheckForUpdates),
                        ),
                        AppUpdateState::Checking => (
                            format!("Current version: v{}", crate::APP_VERSION),
                            container(
                                widget::text(tr(language, "Checking…"))
                                    .size(12)
                                    .color(accent_2()),
                            )
                            .padding([8, 10])
                            .style(chip_style(accent_2()))
                            .into(),
                        ),
                        AppUpdateState::UpToDate { latest_version } => (
                            format!(
                                "Up to date · v{} (latest: v{latest_version})",
                                crate::APP_VERSION
                            ),
                            secondary_button(tr(language, "Check now"), Message::CheckForUpdates),
                        ),
                        AppUpdateState::Available {
                            latest_version,
                            release_url,
                        } => (
                            format!(
                                "Update available · v{} → v{latest_version}",
                                crate::APP_VERSION
                            ),
                            primary_button(
                                tr(language, "Download update"),
                                Message::OpenAppUpdate(release_url.clone()),
                            ),
                        ),
                        AppUpdateState::Failed(error) => (
                            format!("Update check failed: {error}"),
                            secondary_button(tr(language, "Check now"), Message::CheckForUpdates),
                        ),
                    };

                let content = widget::column![
                    Space::new().height(Length::Fixed(
                        (1.0 - eased(self.page_reveal)) * 4.0
                    )),
                    container(
                        widget::row![
                            section_title(
                                tr(language, "Settings"),
                                tr(language, "Tune model behaviour, prompt selection, and chat preferences.")
                            ),
                            Space::new().width(Length::Fill),
                            secondary_button(tr(language, "Go back"), Message::ToggleSettings),
                        ]
                    )
                    .padding(18)
                    .width(Length::Fill)
                    .style(top_bar_style),

                    Space::new().height(Length::Fixed(14.0)),

                    container(
                        widget::row![
                            widget::column![
                                Space::new().height(Length::Fixed(
                                    (1.0 - eased(self.page_reveal * 1.12)) * 14.0
                                )),
                                widget::column![
                            settings_group_title(tr(language, "PERSONALIZATION")),
                            Space::new().height(Length::Fixed(8.0)),
                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Interface language"),
                                        tr(language, "Spanish is experimental and machine-generated. It will be replaced with a human translation in a future update.")
                                    ),
                                    widget::pick_list(
                                        [Language::English, Language::Spanish],
                                        Some(language),
                                        Message::LanguageChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Dynamic system prompt"),
                                        tr(language, "Add current local information and your own instructions to every request. Each profile can add its own user name and instructions; profile names stay private. Switch profiles from the bottom-left corner.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::checkbox(self.dynamic_prompt_settings.include_date)
                                            .label(tr(language, "Include date"))
                                            .on_toggle(|_| Message::ToggleDynamicDate),
                                        Space::new().width(Length::Fill),
                                        widget::checkbox(self.dynamic_prompt_settings.include_time)
                                            .label(tr(language, "Include time"))
                                            .on_toggle(|_| Message::ToggleDynamicTime),
                                    ],
                                    Space::new().height(Length::Fixed(10.0)),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "Custom instructions appended to the system prompt"),
                                        &self.dynamic_prompt_settings.custom_instructions,
                                    )
                                    .on_input(Message::DynamicCustomInstructionsChanged)
                                    .padding(11)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "MODEL & RESPONSES")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Maximum response"),
                                        tr(language, "Caps generated output in tokens, including hidden reasoning. The default is 32,768; direct entry supports up to 1,048,576.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            9.0..=20.0,
                                            self.user_information.max_response_tokens.ilog2() as f32,
                                            Message::UpdateMaxResponseTokens,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        iced::widget::TextInput::<Message>::new(
                                            "tokens",
                                            &self.max_response_tokens_input,
                                        )
                                        .on_input(Message::EditMaxResponseTokens)
                                        .on_submit(Message::ApplyMaxResponseTokens)
                                        .padding(
                                            9.0 + self.settings_feedback(
                                                SettingsFeedbackTarget::MaxResponse
                                            ) * 0.8
                                        )
                                        .width(Length::Fixed(130.0))
                                        .style(feedback_text_input_style(
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::MaxResponse
                                            )
                                        )),
                                        feedback_apply_button(
                                            "Apply",
                                            Message::ApplyMaxResponseTokens,
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ApplyMaxResponse
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Context window"),
                                        tr(language, "Controls how much conversation and generated output the model can hold. Larger values use substantially more memory.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            12.0..=22.0,
                                            self.user_information.context_tokens.ilog2() as f32,
                                            Message::UpdateContextTokens,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        iced::widget::TextInput::<Message>::new(
                                            "tokens",
                                            &self.context_tokens_input,
                                        )
                                        .on_input(Message::EditContextTokens)
                                        .on_submit(Message::ApplyContextTokens)
                                        .padding(
                                            9.0 + self.settings_feedback(
                                                SettingsFeedbackTarget::ContextWindow
                                            ) * 0.8
                                        )
                                        .width(Length::Fixed(130.0))
                                        .style(feedback_text_input_style(
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ContextWindow
                                            )
                                        )),
                                        feedback_apply_button(
                                            "Apply",
                                            Message::ApplyContextTokens,
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ApplyContextWindow
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Model"),
                                        tr(language, "Choose the Ollama model used for new responses.")
                                    ),
                                    widget::pick_list(
                                        bots_list,
                                        self.user_information.model.clone(),
                                        Message::ModelChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            if self.user_information.thinking_supported == Some(true) {
                                container(
                                    widget::column![
                                        setting_label(
                                            tr(language, "Thinking effort"),
                                            tr(language, "Choose how much reasoning the model should use.")
                                        ),
                                        Space::new().height(Length::Fixed(10.0)),
                                        thinking_control(
                                            self.user_information.thinking_level,
                                            &self.user_information.thinking_levels,
                                            language,
                                        ),
                                    ]
                                )
                                .padding(16)
                                .width(Length::Fill)
                                .style(flat_card_style)
                            } else {
                                container(
                                    setting_label(
                                        tr(language, "Reasoning"),
                                        if self.user_information.thinking_supported == Some(false) {
                                            tr(language, "This model does not offer adjustable reasoning.")
                                        } else {
                                            tr(language, "Select a model and wait while reasoning support is checked.")
                                        }
                                    )
                                )
                                .padding(16)
                                .width(Length::Fill)
                                .style(flat_card_style)
                            },

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Temperature"),
                                        tr(language, "Higher values make output more random.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            0.0..=10.0,
                                            self.user_information.temperature,
                                            Message::UpdateTemperature,
                                        ),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            format!("{:.1}", self.user_information.temperature),
                                            accent(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::Temperature
                                            ),
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "System prompt"),
                                        tr(language, "Choose the personality or instruction profile.")
                                    ),
                                    widget::pick_list(
                                        prompts_list,
                                        self.system_prompt.system_prompt.clone(),
                                        Message::SystemPromptChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                                ]
                            ]
                            .width(Length::Fill),

                            Space::new().width(Length::Fixed(14.0)),

                            widget::column![
                                Space::new().height(Length::Fixed(
                                    (1.0 - eased(self.page_reveal * 1.18 - 0.08)) * 22.0
                                )),
                                widget::column![
                            settings_group_title(tr(language, "APPEARANCE")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    setting_label(
                                        tr(language, "Text size"),
                                        tr(language, "Adjust chat and response readability.")
                                    ),
                                    Space::new().height(Length::Fixed(10.0)),
                                    widget::row![
                                        widget::slider(
                                            1.0..=40.0,
                                            self.user_information.text_size,
                                            Message::UpdateTextSize,
                                        ),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            format!("{:.0}px", self.user_information.text_size),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::TextSize
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(14.0)),
                                    setting_label(
                                        tr(language, "Chat font"),
                                        tr(language, "Choose the font family used for prompts, responses, and reasoning.")
                                    ),
                                    Space::new().height(Length::Fixed(8.0)),
                                    widget::pick_list(
                                        FontFamily::ALL,
                                        Some(self.user_information.font_family),
                                        Message::FontFamilyChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::row![
                                    setting_label(
                                        tr(language, "Dark mode"),
                                        tr(language, "Switch between the dark and light interface themes.")
                                    ),
                                    widget::checkbox(self.app_state.dark_mode)
                                        .label(tr(language, "Enabled"))
                                        .on_toggle(|_| Message::ToggleDarkMode),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "TOOLS")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    widget::row![
                                        setting_label(
                                            tr(language, "Enable Tools"),
                                            tr(language, "Allow the model to use tools. When disabled, the model responds directly without tool calls.")
                                        ),
                                        widget::checkbox(self.tool_settings.enabled)
                                            .label(tr(language, "Enabled"))
                                            .on_toggle(|_| Message::ToggleTools),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Available tools"),
                                        tr(language, "Choose which tools the model can use when tools are enabled.")
                                    ),
                                    Space::new().height(Length::Fixed(8.0)),
                                    widget::row![
                                        widget::checkbox(
                                            self.tool_settings.web_search
                                        )
                                        .label(tr(language, "Web Search"))
                                        .on_toggle(|_| Message::ToggleWebSearchTool),
                                        Space::new().width(Length::Fixed(16.0)),
                                        widget::checkbox(
                                            self.tool_settings.fetch_webpage
                                        )
                                        .label(tr(language, "Page Fetch"))
                                        .on_toggle(|_| Message::ToggleFetchWebpageTool),
                                        Space::new().width(Length::Fixed(16.0)),
                                        widget::checkbox(
                                            self.tool_settings.conversation_search
                                        )
                                        .label(tr(language, "Past Chats"))
                                        .on_toggle(|_| Message::ToggleConversationSearchTool),
                                        Space::new().width(Length::Fixed(16.0)),
                                        widget::checkbox(
                                            self.tool_settings.code_checking
                                        )
                                        .label(tr(language, "Code Checking"))
                                        .on_toggle(|_| Message::ToggleCodeCheckingTool),
                                    ],
                                    Space::new().height(Length::Fixed(8.0)),
                                    widget::text(tr(language, "Code Checking also requires Local code checking in Advanced settings."))
                                        .size(12)
                                        .color(text_muted()),
                                ]
                            )
                            .padding(20)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "WEB SEARCH")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::column![
                                    widget::row![
                                        setting_label(
                                            tr(language, "Enable Web Search"),
                                            tr(language, "Web search may send search queries and webpage URLs to the selected external provider.")
                                        ),
                                        widget::checkbox(self.web_search_settings.enabled)
                                            .label(tr(language, "Enabled"))
                                            .on_toggle(|_| Message::ToggleWebSearch),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Search provider"),
                                        "The provider is contacted only while web search is enabled. Tavily and Exa support is experimental."
                                    ),
                                    widget::pick_list(
                                        crate::tools::web_search::WebSearchProviderKind::ALL,
                                        Some(self.web_search_settings.provider),
                                        Message::WebSearchProviderChange,
                                    )
                                    .padding([12, 14])
                                    .text_size(14)
                                    .style(pick_list_style)
                                    .menu_style(pick_list_menu_style)
                                    .width(Length::Fill),
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "API key"),
                                        self.web_search_settings.provider.api_key_help()
                                    ),
                                    iced::widget::TextInput::<Message>::new(
                                        self.web_search_settings.provider.api_key_placeholder(),
                                        &web_api_key,
                                    )
                                    .secure(true)
                                    .padding(12)
                                    .on_input(Message::WebSearchApiKeyChange)
                                    .style(text_input_style),
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Search result limit"),
                                        "Limits results returned by each model-requested search."
                                    ),
                                    widget::row![
                                        widget::slider(
                                            1.0..=crate::tools::web_search::MAX_RESULT_LIMIT as f32,
                                            self.web_search_settings.result_limit as f32,
                                            Message::WebSearchResultLimitChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.result_limit.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::SearchResultLimit
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    widget::row![
                                        setting_label(
                                            tr(language, "Deep follow-up research"),
                                            tr(language, "Use the configurable multi-query and cross-source research budget below.")
                                        ),
                                        widget::checkbox(
                                            self.web_search_settings.allow_multiple_searches
                                        )
                                        .label(tr(language, "Enabled"))
                                        .on_toggle(|_| Message::ToggleMultipleWebSearches),
                                    ],
                                    Space::new().height(Length::Fixed(16.0)),
                                    settings_disclosure_button(
                                        tr(language, "Deep research controls"),
                                        self.deep_research_controls_open,
                                        Message::ToggleDeepResearchControls,
                                    ),
                                    if self.deep_research_controls_open {
                                    widget::column![
                                    Space::new().height(Length::Fixed(10.0 +
                                        (1.0 - eased(self.page_reveal)) * 4.0
                                    )),
                                    setting_label(
                                        tr(language, "Maximum searches"),
                                        tr(language, "Hard cap on distinct search queries in one deep-research response.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            1.0..=crate::tools::web_search::MAX_CONFIGURABLE_SEARCHES as f32,
                                            self.web_search_settings.maximum_searches as f32,
                                            Message::WebSearchMaximumSearchesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.maximum_searches.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::MaximumSearches
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Required successful searches"),
                                        tr(language, "The model is reminded to keep researching until this many distinct searches succeed.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            1.0..=self.web_search_settings.maximum_searches as f32,
                                            self.web_search_settings.minimum_successful_searches as f32,
                                            Message::WebSearchMinimumSuccessfulSearchesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.minimum_successful_searches.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::RequiredSearches
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Maximum page reads"),
                                        tr(language, "Hard cap on full webpage fetches in one deep-research response. Set to 0 to disable page reads.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            0.0..=crate::tools::web_search::MAX_CONFIGURABLE_PAGES as f32,
                                            self.web_search_settings.maximum_page_fetches as f32,
                                            Message::WebSearchMaximumPageFetchesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.maximum_page_fetches.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::MaximumPageReads
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Required independent pages"),
                                        tr(language, "Minimum number of different domains the model should inspect. Set to 0 to disable this checkpoint.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            0.0..=self.web_search_settings.maximum_page_fetches as f32,
                                            self.web_search_settings.minimum_independent_pages as f32,
                                            Message::WebSearchMinimumIndependentPagesChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.minimum_independent_pages.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::RequiredPageReads
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Maximum tool rounds"),
                                        tr(language, "Caps model/tool hand-offs. Reaching this limit now forces a final answer instead of failing the prompt.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            2.0..=crate::tools::web_search::MAX_CONFIGURABLE_TOOL_ITERATIONS as f32,
                                            self.web_search_settings.tool_iteration_limit as f32,
                                            Message::WebSearchToolIterationLimitChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            self.web_search_settings.tool_iteration_limit.to_string(),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::ToolRounds
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Web request timeout"),
                                        tr(language, "Timeout for each external search or webpage request.")
                                    ),
                                    widget::row![
                                        widget::slider(
                                            3.0..=60.0,
                                            self.web_search_settings.request_timeout_seconds as f32,
                                            Message::WebSearchRequestTimeoutChange,
                                        )
                                        .step(1.0),
                                        Space::new().width(Length::Fixed(12.0)),
                                        feedback_value_chip(
                                            format!(
                                                "{}s",
                                                self.web_search_settings.request_timeout_seconds
                                            ),
                                            accent_2(),
                                            self.settings_feedback(
                                                SettingsFeedbackTarget::RequestTimeout
                                            ),
                                        ),
                                    ],
                                    Space::new().height(Length::Fixed(12.0)),
                                    setting_label(
                                        tr(language, "Custom research instructions"),
                                        tr(language, "Optional guidance added only to deep-research requests.")
                                    ),
                                    iced::widget::TextInput::<Message>::new(
                                        tr(language, "e.g. Prioritize official sources and compare opposing views"),
                                        &self.web_search_settings.custom_research_instructions,
                                    )
                                    .on_input(Message::WebSearchCustomInstructionsChange)
                                    .padding(11)
                                    .width(Length::Fill)
                                    .style(text_input_style),
                                    ]
                                    } else {
                                        widget::column![]
                                    },
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            settings_group_title(tr(language, "DATA & MAINTENANCE")),
                            Space::new().height(Length::Fixed(8.0)),

                            container(
                                widget::row![
                                    widget::column![
                                        setting_label(
                                            tr(language, "Application updates"),
                                            tr(language, "Current version and latest stable release from GitHub.")
                                        ),
                                        widget::text(update_status)
                                            .size(12)
                                            .color(text_muted())
                                            .wrapping(Wrapping::WordOrGlyph),
                                    ]
                                    .width(Length::Fill),
                                    update_action,
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::row![
                                    widget::column![
                                        setting_label(
                                            tr(language, "Chat storage"),
                                            tr(language, "Saved chats use this folder. The full path is shown so you can always locate them.")
                                        ),
                                        widget::text(self.chat_storage_dir.display().to_string())
                                            .size(12)
                                            .color(text_muted())
                                            .wrapping(Wrapping::WordOrGlyph),
                                    ]
                                    .width(Length::Fill),
                                    secondary_button(
                                        tr(language, "Choose folder"),
                                        Message::ChooseChatFolder
                                    ),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(10.0)),

                            container(
                                widget::row![
                                    setting_label(
                                        tr(language, "Model conversation context"),
                                        tr(language, "Include earlier messages from this chat in the next model request. Saved chats are managed in the left menu.")
                                    ),
                                    widget::checkbox(
                                        user_information.current_chat_history_enabled
                                    )
                                    .label(tr(language, "Enabled"))
                                    .on_toggle(|_| Message::ToggleChatHistory),
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(flat_card_style),

                            Space::new().height(Length::Fixed(14.0)),

                            container(
                                widget::column![
                                    widget::column![
                                        widget::text(tr(language, "Maintenance"))
                                            .size(16)
                                            .color(text_main()),
                                        Space::new().height(Length::Fixed(4.0)),
                                        widget::text(tr(language, "Clear local conversation data or open deeper configuration options."))
                                            .size(12)
                                            .color(text_muted()),
                                    ]
                                    .width(Length::Fill),

                                    Space::new().height(Length::Fixed(12.0)),

                                    widget::row![
                                        danger_button(
                                            tr(language, "Clear current context"),
                                            Message::WipeChatHistory
                                        ),

                                        Space::new().width(Length::Fill),

                                        secondary_button(
                                            tr(language, "Advanced settings"),
                                            Message::ToggleAdvancedSettings
                                        ),
                                    ],
                                ]
                            )
                            .padding(16)
                            .width(Length::Fill)
                            .style(danger_zone_style),

                            Space::new().height(Length::Fixed(12.0)),

                            widget::text(self.debug_message.clone().message)
                                .size(13)
                                .color(debug_color),
                                ]
                            ]
                            .width(Length::Fill),
                        ]
                    )
                    .padding(18)
                    .width(Length::Fill)
                    .style(conversation_style),
                ];

                container(widget::scrollable(content).height(Length::Fill))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }

            GUIState::AdvancedSettings => {
                let user_information = self.user_information.clone();
                let ip = self.user_information.ip_address.clone();

                let prompts_list = self
                    .system_prompt
                    .system_prompts_as_vec
                    .lock()
                    .unwrap()
                    .clone();

                let model_install = iced::widget::TextInput::<Message>::new(
                    tr(language, "Model name, e.g. llama3.2:3b"),
                    &self.installing_model,
                )
                .padding(12)
                .size(15)
                .width(Length::Fill)
                .on_submit(Message::InstallModel(self.installing_model.clone()))
                .on_input(Message::UpdateInstall)
                .style(text_input_style);

                let change_ip = iced::widget::TextInput::<Message>::new(ip.ip.as_str(), &ip.ip)
                    .padding(12)
                    .size(15)
                    .width(Length::FillPortion(3))
                    .on_submit(Message::ChangeIp(ip.ip.clone()))
                    .on_input(Message::ChangeIp)
                    .style(text_input_style);

                let change_protocol =
                    iced::widget::TextInput::<Message>::new("https", &ip.protocol)
                        .padding(12)
                        .size(15)
                        .width(Length::Fixed(88.0))
                        .on_submit(Message::ChangeProtocol(ip.protocol.clone()))
                        .on_input(Message::ChangeProtocol)
                        .style(text_input_style);

                let change_port =
                    iced::widget::TextInput::<Message>::new(ip.port.as_str(), &ip.port)
                        .padding(12)
                        .size(15)
                        .width(Length::FillPortion(1))
                        .on_submit(Message::ChangePort(ip.port.clone()))
                        .on_input(Message::ChangePort)
                        .style(text_input_style);

                let content = widget::column![
                    Space::new().height(Length::Fixed(
                        (1.0 - eased(self.page_reveal)) * 4.0
                    )),
                    container(widget::row![
                        section_title(
                            tr(language, "Advanced settings"),
                            tr(language, "Install models, change connection settings, and tune rendering.")
                        ),
                        Space::new().width(Length::Fill),
                        secondary_button(tr(language, "Back to settings"), Message::ToggleAdvancedSettings),
                    ])
                    .padding(18)
                    .width(Length::Fill)
                    .style(top_bar_style),
                    Space::new().height(Length::Fixed(14.0)),
                    container(widget::row![
                        widget::column![
                            Space::new().height(Length::Fixed(
                                (1.0 - eased(self.page_reveal * 1.12)) * 14.0
                            )),
                            settings_group_title(tr(language, "MODELS & SAFETY")),
                            Space::new().height(Length::Fixed(8.0)),
                            widget::column![
                        container(widget::column![
                            setting_label(tr(language, "System prompt"), tr(language, "Change the active prompt profile.")),
                            widget::pick_list(
                                prompts_list,
                                self.system_prompt.system_prompt.clone(),
                                Message::SystemPromptChange,
                            )
                            .padding([12, 14])
                            .text_size(14)
                            .style(pick_list_style)
                            .menu_style(pick_list_menu_style)
                            .width(Length::Fill),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::column![
                            widget::row![
                                setting_label(
                                    tr(language, "Local code checking"),
                                    tr(language, "Allow generated Python, Rust, C, C++, and C# snippets to be checked with locally installed tools.")
                                ),
                                widget::checkbox(self.code_checking_enabled)
                                    .label(tr(language, "Enabled"))
                                    .on_toggle(|_| Message::ToggleCodeChecking),
                            ],
                            Space::new().height(Length::Fixed(8.0)),
                            widget::text(tr(language, "Warning: checking invokes local compilers or interpreters on generated code. It can fail, consume resources, or cause unintended errors. Enable it only when you consent and trust the code."))
                                .size(12)
                                .color(warning()),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::column![
                            setting_label(
                                tr(language, "Install model"),
                                tr(language, "Enter an Ollama model name and press Enter.")
                            ),
                            model_install,
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                            ]
                        ]
                        .width(Length::Fill),

                        Space::new().width(Length::Fixed(14.0)),

                        widget::column![
                            Space::new().height(Length::Fixed(
                                (1.0 - eased(self.page_reveal * 1.18 - 0.08)) * 22.0
                            )),
                            settings_group_title(tr(language, "RUNTIME & CONNECTION")),
                            Space::new().height(Length::Fixed(8.0)),
                            widget::column![
                        container(widget::column![
                            setting_label(
                                tr(language, "Batch tokens"),
                                tr(language, "Tokens per visual update when fast streaming is off. Higher values reduce rendering work.")
                            ),
                            Space::new().height(Length::Fixed(10.0)),
                            widget::row![
                                widget::slider(1.0..=10.0, self.batch_tokens as f32, |value| {
                                    Message::ChangeBatchTokens(value as i32)
                                },),
                                Space::new().width(Length::Fixed(12.0)),
                                feedback_value_chip(
                                    self.batch_tokens.to_string(),
                                    accent(),
                                    self.settings_feedback(SettingsFeedbackTarget::BatchTokens),
                                ),
                            ],
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Fast streaming"),
                                tr(language, "Render as soon as the API yields output. Turn off to use token batching.")
                            ),
                            widget::checkbox(self.fast_streaming)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleFastStreaming),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Show tokens per second at bottom of message"),
                                tr(language, "Display the generation speed under each assistant reply. Measured from the model's own statistics, independent of token batching.")
                            ),
                            widget::checkbox(self.show_tokens_per_second)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleShowTokensPerSecond),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Content filtering"),
                                tr(language, "Censor offensive, profane, sexual, and severely inappropriate words with # characters.")
                            ),
                            widget::checkbox(self.app_state.filtering)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleFiltering),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::row![
                            setting_label(
                                tr(language, "Show info popup on startup"),
                                tr(language, "Display the informational overview when the app launches. You can still open it anytime from the info button.")
                            ),
                            widget::checkbox(self.show_info_popup)
                                .label(tr(language, "Enabled"))
                                .on_toggle(|_| Message::ToggleInfoPopupSetting),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                        Space::new().height(Length::Fixed(10.0)),
                        container(widget::column![
                            setting_label(
                                tr(language, "Ollama address"),
                                tr(language, "Choose HTTP or HTTPS, then enter the hostname or IP address and port used to connect to Ollama.")
                            ),
                            Space::new().height(Length::Fixed(12.0)),
                            widget::row![
                                change_protocol,
                                widget::text("://").size(20).color(text_muted()),
                                change_ip,
                                Space::new().width(Length::Fixed(8.0)),
                                widget::text(":").size(20).color(text_muted()),
                                Space::new().width(Length::Fixed(8.0)),
                                change_port,
                            ],
                            Space::new().height(Length::Fixed(12.0)),
                            container(
                                widget::text(if language == Language::Spanish {
                                    format!(
                                        "Dirección actual: {}://{}:{}",
                                        user_information.ip_address.protocol,
                                        user_information.ip_address.ip,
                                        user_information.ip_address.port
                                    )
                                } else {
                                    format!(
                                        "Current address: {}://{}:{}",
                                        user_information.ip_address.protocol,
                                        user_information.ip_address.ip,
                                        user_information.ip_address.port
                                    )
                                })
                                .size(13)
                                .color(text_main())
                            )
                            .padding(10)
                            .style(chip_style(accent_2())),
                        ])
                        .padding(16)
                        .width(Length::Fill)
                        .style(flat_card_style),
                            ]
                        ]
                        .width(Length::Fill),
                    ])
                    .padding(18)
                    .width(Length::Fill)
                    .style(conversation_style),
                ];

                container(widget::scrollable(content).height(Length::Fill))
                    .padding(18)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(app_background_style)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ellipsize_chat_title;

    #[test]
    fn chat_title_ellipsis_is_unicode_safe() {
        assert_eq!(ellipsize_chat_title("🦀 Rustaceans unite", 6), "🦀 Rus…");
    }

    #[test]
    fn chat_title_ellipsis_keeps_short_titles_unchanged() {
        assert_eq!(ellipsize_chat_title("Short title", 20), "Short title");
        assert_eq!(ellipsize_chat_title("12345678", 8), "12345678");
    }

    #[test]
    fn chat_title_ellipsis_forces_titles_onto_one_line() {
        assert_eq!(
            ellipsize_chat_title("first line\nsecond\tline", 18),
            "first line second…"
        );
        assert_eq!(ellipsize_chat_title("anything", 0), "");
    }
}
