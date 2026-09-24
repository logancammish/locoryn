use super::{theme::*, translation::tr};
use crate::{
    FilePreview, Language, Message,
    file_usage::{AttachedFile, excerpt},
};
use iced::{
    Element, Length,
    widget::{self, Space, container},
};
use std::sync::Arc;

pub(super) fn file_chips(
    files: &[Arc<AttachedFile>],
    removable: bool,
    language: Language,
) -> Element<'_, Message> {
    if files.is_empty() {
        return Space::new().width(0).height(0).into();
    }
    let chips = files.iter().enumerate().map(|(index, file)| {
        let label = format!(
            "{}{}",
            if file.has_notices() { "⚠ " } else { "" },
            file.summary()
        );
        let preview = mini_button_owned(label, Message::PreviewFile(Arc::clone(file)));
        if removable {
            widget::row![
                preview,
                compact_icon_button("×", tr(language, "Remove file"), Message::RemoveFile(index))
            ]
            .align_y(iced::Alignment::Center)
            .into()
        } else {
            preview
        }
    });
    widget::scrollable(widget::Row::with_children(chips).spacing(4))
        .direction(widget::scrollable::Direction::Horizontal(
            widget::scrollable::Scrollbar::new(),
        ))
        .height(Length::Fixed(38.0))
        .into()
}

pub(super) fn file_preview<'a>(
    preview: &'a FilePreview,
    width: f32,
    height: f32,
    language: Language,
) -> Element<'a, Message> {
    let file = &preview.file;
    let Some(page) = file.pages.get(preview.page) else {
        return Space::new().into();
    };
    let (text, next_offset) = excerpt(&page.text, preview.offset, 12_000);
    let notice = page.notice.as_deref().unwrap_or(if page.ocr {
        tr(language, "OCR text — recognition may contain errors.")
    } else {
        tr(language, "Extracted text")
    });
    let body = container(
        widget::column![
            widget::row![
                widget::text(file.name.as_str())
                    .size(16)
                    .width(Length::Fill),
                compact_icon_button("×", tr(language, "Close"), Message::CloseFilePreview),
            ]
            .align_y(iced::Alignment::Center),
            widget::text(notice).size(12).color(text_muted()),
            widget::scrollable(
                widget::text(text)
                    .size(14)
                    .font(iced::Font::MONOSPACE)
                    .wrapping(iced_widget::core::text::Wrapping::WordOrGlyph)
            )
            .height(Length::Fill),
            widget::row![
                compact_icon_button(
                    "‹",
                    tr(language, "Previous page"),
                    preview.page.checked_sub(1).map(Message::FilePreviewPage)
                ),
                widget::text(format!(
                    "{} {} / {}",
                    tr(language, "Page"),
                    preview.page + 1,
                    file.pages.len()
                ))
                .size(13),
                compact_icon_button(
                    "›",
                    tr(language, "Next page"),
                    (preview.page + 1 < file.pages.len())
                        .then_some(Message::FilePreviewPage(preview.page + 1))
                ),
                Space::new().width(Length::Fill),
                compact_icon_button(
                    "←",
                    tr(language, "Previous excerpt"),
                    (preview.offset > 0).then_some(Message::FilePreviewOffset(
                        preview.offset.saturating_sub(12_000)
                    ))
                ),
                compact_icon_button(
                    "→",
                    tr(language, "Next excerpt"),
                    next_offset.map(Message::FilePreviewOffset)
                ),
            ]
            .spacing(6)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(10),
    )
    .padding(18)
    .width(Length::Fixed((width - 48.0).clamp(260.0, 880.0)))
    .height(Length::Fixed((height - 70.0).clamp(240.0, 720.0)))
    .style(flat_card_style);
    // The opaque interaction layer keeps clicks/scrolling in the preview.
    widget::stack![
        widget::opaque(
            widget::mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
                .on_press(Message::CloseFilePreview)
        ),
        container(widget::opaque(body)).center(Length::Fill),
    ]
    .into()
}
