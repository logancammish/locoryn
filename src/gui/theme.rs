use super::*;

pub(super) fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::from_rgb(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
}

static INTERFACE_THEME: AtomicU8 = AtomicU8::new(0);

#[cfg(target_os = "windows")]
const CHAT_SERIF_FONT: &str = "Times New Roman";
#[cfg(target_os = "macos")]
const CHAT_SERIF_FONT: &str = "Times";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const CHAT_SERIF_FONT: &str = "DejaVu Serif";

#[cfg(target_os = "windows")]
const CHAT_MONOSPACE_FONT: &str = "Consolas";
#[cfg(target_os = "macos")]
const CHAT_MONOSPACE_FONT: &str = "Menlo";
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const CHAT_MONOSPACE_FONT: &str = "DejaVu Sans Mono";

pub(crate) fn set_interface_theme(theme: InterfaceTheme) {
    let value = match theme {
        InterfaceTheme::Dark => 0,
        InterfaceTheme::Light => 1,
        InterfaceTheme::Modern => 2,
    };
    INTERFACE_THEME.store(value, Ordering::Relaxed);
}

pub(super) fn interface_theme() -> InterfaceTheme {
    match INTERFACE_THEME.load(Ordering::Relaxed) {
        1 => InterfaceTheme::Light,
        2 => InterfaceTheme::Modern,
        _ => InterfaceTheme::Dark,
    }
}

pub(super) fn is_dark_mode() -> bool {
    interface_theme() != InterfaceTheme::Light
}

pub(super) fn chat_font(font_family: FontFamily) -> iced::Font {
    match font_family {
        FontFamily::SansSerif => iced::Font::with_name("Fira Sans"),
        FontFamily::Serif => iced::Font::with_name(CHAT_SERIF_FONT),
        FontFamily::Monospace => iced::Font::with_name(CHAT_MONOSPACE_FONT),
    }
}

pub(super) fn app_bg() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(7, 9, 14),
        InterfaceTheme::Modern => rgb(8, 10, 15),
        InterfaceTheme::Light => rgb(246, 247, 251),
    }
}

pub(super) fn panel() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(15, 19, 29),
        InterfaceTheme::Modern => rgb(15, 18, 26),
        InterfaceTheme::Light => rgb(255, 255, 255),
    }
}

pub(super) fn panel_soft() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(21, 26, 39),
        InterfaceTheme::Modern => rgb(20, 24, 34),
        InterfaceTheme::Light => rgb(248, 249, 252),
    }
}

pub(super) fn panel_lifted() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(28, 34, 49),
        InterfaceTheme::Modern => rgb(25, 30, 42),
        InterfaceTheme::Light => rgb(252, 252, 254),
    }
}

pub(super) fn border_soft() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(50, 61, 84),
        InterfaceTheme::Modern => rgb(42, 48, 64),
        InterfaceTheme::Light => rgb(222, 225, 234),
    }
}

pub(super) fn border_bright() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(82, 102, 145),
        InterfaceTheme::Modern => rgb(79, 88, 116),
        InterfaceTheme::Light => rgb(164, 170, 190),
    }
}

pub(super) fn text_main() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(240, 245, 255),
        InterfaceTheme::Modern => rgb(241, 242, 247),
        InterfaceTheme::Light => rgb(29, 31, 40),
    }
}

pub(super) fn text_muted() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(158, 170, 195),
        InterfaceTheme::Modern => rgb(161, 167, 184),
        InterfaceTheme::Light => rgb(91, 95, 111),
    }
}

pub(super) fn text_faint() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(110, 122, 148),
        InterfaceTheme::Modern => {
            // 6.1:1 against the modern application background for small metadata.
            rgb(146, 152, 169)
        }
        InterfaceTheme::Light => rgb(82, 87, 103),
    }
}

pub(super) fn accent() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(82, 140, 255),
        InterfaceTheme::Light | InterfaceTheme::Modern => rgb(139, 124, 246),
    }
}

pub(super) fn accent_2() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(108, 226, 209),
        InterfaceTheme::Light | InterfaceTheme::Modern => rgb(87, 214, 198),
    }
}

/// Accent used by navigation and control labels.
///
/// The legacy Dark theme used blue for these affordances. Modern and Light
/// retain their existing secondary accent so this distinction stays scoped to
/// the restored colour scheme.
fn control_accent_for(theme: InterfaceTheme) -> Color {
    match theme {
        InterfaceTheme::Dark => rgb(82, 140, 255),
        InterfaceTheme::Light | InterfaceTheme::Modern => rgb(87, 214, 198),
    }
}

pub(super) fn control_accent() -> Color {
    control_accent_for(interface_theme())
}

pub(super) fn danger() -> Color {
    rgb(255, 92, 116)
}

pub(super) fn success() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(93, 225, 144),
        InterfaceTheme::Light | InterfaceTheme::Modern => rgb(91, 211, 157),
    }
}

pub(super) fn warning() -> Color {
    rgb(255, 190, 94)
}

pub(super) fn shadow_color() -> Color {
    Color {
        a: if is_dark_mode() { 0.30 } else { 0.12 },
        ..rgb(0, 0, 0)
    }
}

fn sidebar_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => panel(),
        InterfaceTheme::Modern => rgb(12, 15, 22),
        InterfaceTheme::Light => rgb(251, 251, 253),
    }
}

fn conversation_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => panel(),
        InterfaceTheme::Modern => rgb(11, 14, 20),
        InterfaceTheme::Light => rgb(252, 252, 254),
    }
}

fn active_control_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(34, 49, 80),
        InterfaceTheme::Modern => rgb(29, 37, 57),
        InterfaceTheme::Light => rgb(232, 237, 248),
    }
}

fn selected_control_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(48, 93, 190),
        InterfaceTheme::Modern => rgb(49, 67, 122),
        InterfaceTheme::Light => rgb(75, 99, 205),
    }
}

fn active_chat_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(34, 49, 80),
        InterfaceTheme::Modern => rgb(29, 28, 45),
        InterfaceTheme::Light => rgb(243, 241, 250),
    }
}

fn user_bubble_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(48, 93, 190),
        InterfaceTheme::Modern => rgb(56, 48, 101),
        InterfaceTheme::Light => rgb(235, 231, 252),
    }
}

fn bot_bubble_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(23, 28, 42),
        InterfaceTheme::Modern => rgb(17, 21, 29),
        InterfaceTheme::Light => rgb(255, 255, 255),
    }
}

fn primary_button_colors() -> (Color, Color) {
    match interface_theme() {
        InterfaceTheme::Dark => (rgb(70, 125, 255), rgb(107, 158, 255)),
        InterfaceTheme::Modern => (rgb(111, 91, 218), accent()),
        InterfaceTheme::Light => (rgb(112, 91, 218), accent()),
    }
}

fn suggestion_hover_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(34, 49, 80),
        InterfaceTheme::Modern => rgb(32, 35, 51),
        InterfaceTheme::Light => rgb(246, 244, 253),
    }
}

fn assistant_mark_background() -> Color {
    match interface_theme() {
        InterfaceTheme::Dark => rgb(70, 125, 255),
        InterfaceTheme::Modern => rgb(104, 88, 205),
        InterfaceTheme::Light => rgb(117, 98, 221),
    }
}

pub(super) fn with_alpha(color: Color, alpha: f32) -> Color {
    Color {
        a: alpha.clamp(0.0, 1.0),
        ..color
    }
}

pub(super) fn mix_color(from: Color, to: Color, amount: f32) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    Color {
        r: from.r + (to.r - from.r) * amount,
        g: from.g + (to.g - from.g) * amount,
        b: from.b + (to.b - from.b) * amount,
        a: from.a + (to.a - from.a) * amount,
    }
}

pub(super) fn eased(value: f32) -> f32 {
    let value = value.clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

pub(super) fn app_background_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(app_bg())),
        border: Border {
            color: app_bg(),
            width: 0.0,
            radius: Radius::from(0.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn sidebar_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(sidebar_background())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(14.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn top_bar_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(14.0),
        },
        shadow: Shadow {
            color: Color {
                a: if is_dark_mode() { 0.18 } else { 0.06 },
                ..rgb(0, 0, 0)
            },
            offset: Vector::from([0.0, 2.0]),
            blur_radius: 10.0,
        },
    }
}

pub(super) fn config_drawer_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel_soft())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(10.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn conversation_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(conversation_background())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(14.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn composer_style(_active: bool, _pulse: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(16.0),
        },
        shadow: Shadow {
            color: with_alpha(shadow_color(), 0.32),
            offset: Vector::from([0.0, 3.0]),
            blur_radius: 12.0,
        },
    }
}

pub(super) fn pick_list_style(
    _theme: &Theme,
    status: widget::pick_list::Status,
) -> widget::pick_list::Style {
    let active = !matches!(status, widget::pick_list::Status::Active);
    widget::pick_list::Style {
        text_color: text_main(),
        placeholder_color: text_faint(),
        handle_color: if active { accent() } else { text_muted() },
        background: Background::Color(if active {
            active_control_background()
        } else {
            panel_soft()
        }),
        border: Border {
            color: if active { accent() } else { border_soft() },
            width: if active { 1.5 } else { 1.0 },
            radius: Radius::from(12.0),
        },
    }
}

pub(super) fn pick_list_menu_style(_theme: &Theme) -> widget::overlay::menu::Style {
    widget::overlay::menu::Style {
        background: Background::Color(panel_lifted()),
        border: Border {
            color: border_bright(),
            width: 1.0,
            radius: Radius::from(14.0),
        },
        text_color: text_main(),
        selected_text_color: Color::WHITE,
        selected_background: Background::Color(selected_control_background()),
        shadow: Shadow {
            color: shadow_color(),
            offset: Vector::from([0.0, 8.0]),
            blur_radius: 22.0,
        },
    }
}

pub(super) fn flat_card_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel_lifted())),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(14.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn chat_entry_style(active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if active {
            active_chat_background()
        } else {
            Color::TRANSPARENT
        })),
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow::default(),
    }
}

/// A short, conventional fade for long chat titles at the trailing action edge.
/// The opaque end matches the entry (or sidebar) background, so the text simply
/// disappears instead of ending abruptly beneath the overflow button.
pub(super) fn chat_title_fade_style(active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme| {
        let end_color = if active {
            active_chat_background()
        } else {
            sidebar_background()
        };

        Style {
            snap: true,
            text_color: None,
            background: Some(Background::Gradient(
                iced::gradient::Linear::new(iced::Degrees(90.0))
                    .add_stop(0.0, Color::TRANSPARENT)
                    .add_stop(1.0, end_color)
                    .into(),
            )),
            border: Border::default(),
            shadow: Shadow::default(),
        }
    }
}

pub(super) fn chat_title_button_style(
    _theme: &Theme,
    status: widget::button::Status,
) -> widget::button::Style {
    widget::button::Style {
        snap: true,
        background: match status {
            widget::button::Status::Hovered => {
                Some(Background::Color(Color::from_rgba8(255, 255, 255, 0.05)))
            }
            _ => None,
        },
        text_color: text_main(),
        border: Border {
            radius: Radius::from(10.0),
            ..Border::default()
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn user_bubble_style(reveal: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| {
        let target = user_bubble_background();
        Style {
            snap: true,
            text_color: Some(if is_dark_mode() {
                Color::WHITE
            } else {
                text_main()
            }),
            background: Some(Background::Color(mix_color(app_bg(), target, reveal))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius::from(16.0),
            },
            shadow: Shadow {
                color: with_alpha(shadow_color(), shadow_color().a * reveal),
                offset: Vector::from([0.0, 2.0 + reveal * 2.0]),
                blur_radius: 4.0 + reveal * 8.0,
            },
        }
    }
}

pub(super) fn bot_bubble_style(reveal: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| {
        let target = bot_bubble_background();
        Style {
            snap: true,
            text_color: Some(text_main()),
            background: Some(Background::Color(mix_color(app_bg(), target, reveal))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius::from(16.0),
            },
            shadow: Shadow {
                color: with_alpha(
                    rgb(0, 0, 0),
                    if is_dark_mode() {
                        0.14 * reveal
                    } else {
                        0.05 * reveal
                    },
                ),
                offset: Vector::from([0.0, 3.0]),
                blur_radius: 4.0 + reveal * 8.0,
            },
        }
    }
}

pub(super) fn web_activity_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(17, 27, 39)
        } else {
            rgb(235, 248, 252)
        })),
        border: Border {
            color: rgb(48, 112, 139),
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn website_row_style(active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if active {
            if is_dark_mode() {
                rgb(27, 55, 68)
            } else {
                rgb(219, 242, 247)
            }
        } else {
            panel_soft()
        })),
        border: Border {
            color: if active { accent_2() } else { border_soft() },
            width: 1.0,
            radius: Radius::from(10.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn chip_style(color: Color) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel_soft())),
        border: Border {
            color,
            width: 1.0,
            radius: Radius::from(999.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn feedback_chip_style(color: Color, bounce: f32) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(mix_color(
            panel_soft(),
            color,
            bounce * 0.08,
        ))),
        border: Border {
            color: brighten(color, bounce * 0.08),
            width: 1.0 + bounce * 0.65,
            radius: Radius::from(999.0),
        },
        shadow: Shadow {
            color: with_alpha(color, bounce * 0.24),
            offset: Vector::from([0.0, bounce * 1.8]),
            blur_radius: bounce * 8.0,
        },
    }
}

pub(super) fn feedback_value_chip(
    value: String,
    color: Color,
    bounce: f32,
) -> Element<'static, Message> {
    container(widget::text(value).size(13).color(text_main()))
        .padding(8.0 + bounce * 1.1)
        .style(feedback_chip_style(color, bounce))
        .into()
}

pub(super) fn feedback_apply_button<'a>(
    label: &'a str,
    message: Message,
    bounce: f32,
) -> Element<'a, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([8.0 + bounce * 0.22, 10.0 + bounce * 0.35])
        .style(move |_theme, status| {
            let mut style = button_visual(panel_soft(), border_soft(), text_muted(), status);
            style.border.color = mix_color(style.border.color, control_accent(), bounce * 0.24);
            style.border.width += bounce * 0.16;
            style.shadow.color = with_alpha(control_accent(), bounce * 0.10);
            style.shadow.offset = Vector::from([0.0, 1.0 + bounce * 0.55]);
            style.shadow.blur_radius += bounce * 2.0;
            style
        })
        .on_press(message)
        .into()
}

/// The brand mark is intentionally static and squircle-shaped: a square with
/// heavily rounded corners, never a circle.
pub(super) fn status_brand_style(color: Color) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: None,
        background: Some(Background::Color(with_alpha(color, 0.035))),
        border: Border {
            color: brighten(color, 0.04),
            width: 1.2,
            radius: Radius::from(10.0),
        },
        shadow: Shadow {
            color: with_alpha(color, 0.13),
            offset: Vector::from([0.0, 1.0]),
            blur_radius: 5.0,
        },
    }
}

pub(super) fn profile_chip_style(
    open: bool,
) -> impl Fn(&Theme, widget::button::Status) -> widget::button::Style {
    move |_theme, status| {
        let hovered = matches!(status, widget::button::Status::Hovered);
        widget::button::Style {
            snap: true,
            background: Some(Background::Color(if open || hovered {
                panel_lifted()
            } else {
                panel()
            })),
            text_color: text_main(),
            border: Border {
                color: border_soft(),
                width: 1.0,
                radius: Radius::from(10.0),
            },
            shadow: Shadow {
                color: with_alpha(shadow_color(), 0.28),
                offset: Vector::from([0.0, 2.0]),
                blur_radius: 6.0,
            },
        }
    }
}

pub(super) fn profile_popup_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(panel_lifted())),
        border: Border {
            color: border_bright(),
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow {
            color: with_alpha(shadow_color(), 0.62),
            offset: Vector::from([0.0, 10.0]),
            blur_radius: 24.0,
        },
    }
}

pub(super) fn resize_rail_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: None,
        background: Some(Background::Color(with_alpha(border_bright(), 0.62))),
        border: Border {
            color: with_alpha(accent(), 0.18),
            width: 1.0,
            radius: Radius::from(999.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 6.0;

pub(super) fn sidebar_resize_handle<'a>() -> Element<'a, Message> {
    widget::mouse_area(
        container(
            container(Space::new())
                .width(Length::Fixed(2.0))
                .height(Length::Fixed(54.0))
                .style(resize_rail_style),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fixed(SIDEBAR_RESIZE_HANDLE_WIDTH))
        .height(Length::Fill),
    )
    .on_press(Message::StartUiResize(crate::UiResizeTarget::Sidebar))
    .on_release(Message::StopUiResize)
    .interaction(mouse::Interaction::ResizingHorizontally)
    .into()
}

pub(super) fn composer_resize_handle<'a>() -> Element<'a, Message> {
    widget::mouse_area(
        container(
            container(Space::new())
                .width(Length::Fixed(54.0))
                .height(Length::Fixed(2.0))
                .style(resize_rail_style),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .width(Length::Fill)
        .height(Length::Fixed(10.0)),
    )
    .on_press(Message::StartUiResize(crate::UiResizeTarget::Composer))
    .on_release(Message::StopUiResize)
    .interaction(mouse::Interaction::ResizingVertically)
    .into()
}

pub(super) fn danger_zone_style(_theme: &Theme) -> Style {
    Style {
        snap: true,
        text_color: Some(text_main()),
        background: Some(Background::Color(if is_dark_mode() {
            rgb(34, 22, 30)
        } else {
            rgb(255, 242, 245)
        })),
        border: Border {
            color: if is_dark_mode() {
                rgb(118, 56, 74)
            } else {
                rgb(220, 155, 170)
            },
            width: 1.0,
            radius: Radius::from(18.0),
        },
        shadow: Shadow::default(),
    }
}

pub(super) fn brighten(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r + amount).min(1.0),
        g: (color.g + amount).min(1.0),
        b: (color.b + amount).min(1.0),
        a: color.a,
    }
}

pub(super) fn darken(color: Color, amount: f32) -> Color {
    Color {
        r: (color.r - amount).max(0.0),
        g: (color.g - amount).max(0.0),
        b: (color.b - amount).max(0.0),
        a: color.a,
    }
}

pub(super) fn button_visual(
    background: Color,
    border: Color,
    text: Color,
    status: widget::button::Status,
) -> widget::button::Style {
    let (background, border, offset_y, blur_radius) = match status {
        widget::button::Status::Hovered => (
            brighten(background, 0.035),
            brighten(border, 0.045),
            3.0,
            14.0,
        ),
        widget::button::Status::Pressed => {
            (darken(background, 0.045), brighten(border, 0.025), 0.0, 3.0)
        }
        widget::button::Status::Disabled => {
            (darken(background, 0.055), darken(border, 0.055), 0.0, 0.0)
        }
        _ => (background, border, 1.0, 4.0),
    };

    widget::button::Style {
        snap: true,
        background: Some(Background::Color(background)),
        text_color: text,
        border: Border {
            color: border,
            width: 1.0,
            radius: Radius::from(12.0),
        },
        shadow: Shadow {
            color: shadow_color(),
            offset: Vector::from([0.0, offset_y]),
            blur_radius,
        },
    }
}

pub(super) fn primary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(14).align_x(Horizontal::Center))
        .padding([12, 16])
        .style(|_theme, status| {
            let (background, border) = primary_button_colors();
            button_visual(background, border, Color::WHITE, status)
        })
        .height(Length::Fixed(44.0))
        .on_press(message)
        .into()
}

pub(super) fn secondary_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(14).align_x(Horizontal::Center))
        .padding([11, 14])
        .height(Length::Fixed(44.0))
        .style(|_theme, _status| button_visual(panel_soft(), border_soft(), text_main(), _status))
        .on_press(message)
        .into()
}

pub(super) fn danger_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(14).align_x(Horizontal::Center))
        .padding(12)
        .height(Length::Fixed(44.0))
        .style(|_theme, _status| {
            button_visual(rgb(104, 38, 55), rgb(185, 76, 99), Color::WHITE, _status)
        })
        .on_press(message)
        .into()
}

pub(super) fn mini_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([6, 9])
        .height(Length::Fixed(32.0))
        .style(|_theme, _status| button_visual(panel_soft(), border_soft(), text_muted(), _status))
        .on_press(message)
        .into()
}

pub(super) fn mini_danger_button<'a>(label: &'a str, message: Message) -> Element<'a, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([6, 9])
        .height(Length::Fixed(32.0))
        .style(|_theme, status| button_visual(panel_soft(), border_soft(), danger(), status))
        .on_press(message)
        .into()
}

pub(super) fn mini_button_owned(label: String, message: Message) -> Element<'static, Message> {
    widget::button(widget::text(label).size(12).align_x(Horizontal::Center))
        .padding([6, 9])
        .height(Length::Fixed(32.0))
        .style(|_theme, _status| button_visual(panel_soft(), border_soft(), text_muted(), _status))
        .on_press(message)
        .into()
}

pub(super) fn settings_disclosure_button<'a>(
    label: &'a str,
    open: bool,
    message: Message,
) -> Element<'a, Message> {
    widget::button(
        widget::row![
            widget::text(label).size(12).color(text_main()),
            Space::new().width(Length::Fill),
            widget::text(if open { "▾" } else { "▸" })
                .size(13)
                .color(control_accent()),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([10, 12])
    .width(Length::Fill)
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_main(), status))
    .on_press(message)
    .into()
}

pub(super) fn toolbar_button<'a>(
    icon: &'a str,
    label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    widget::button(
        widget::row![
            widget::text(icon)
                .size(14)
                .color(control_accent())
                .align_x(Horizontal::Center),
            Space::new().width(Length::Fixed(7.0)),
            widget::text(label).size(13).color(text_main()),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([8, 10])
    .height(Length::Fixed(40.0))
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_main(), status))
    .on_press(message)
    .into()
}

pub(super) fn icon_button<'a>(
    icon: &'a str,
    tooltip_label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    let button = widget::button(
        container(widget::text(icon).size(16).align_x(Horizontal::Center))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(44.0))
    .height(Length::Fixed(44.0))
    .padding(0)
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_main(), status))
    .on_press(message);

    widget::tooltip(
        button,
        container(widget::text(tooltip_label).size(12).color(text_main()))
            .padding([6, 9])
            .style(flat_card_style),
        widget::tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

/// Small, secondary icon action for dense rows such as saved-chat actions.
/// Primary navigation continues to use `icon_button` for a larger target.
pub(super) fn compact_icon_button<'a>(
    icon: &'a str,
    tooltip_label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    let button = widget::button(
        container(widget::text(icon).size(15).align_x(Horizontal::Center))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(36.0))
    .height(Length::Fixed(36.0))
    .padding(0)
    .style(|_theme, status| button_visual(panel_soft(), border_soft(), text_main(), status))
    .on_press(message);

    widget::tooltip(
        button,
        container(widget::text(tooltip_label).size(12).color(text_main()))
            .padding([6, 9])
            .style(flat_card_style),
        widget::tooltip::Position::Bottom,
    )
    .gap(6)
    .into()
}

pub(super) fn suggestion_button(
    emoji: &'static str,
    label: &'static str,
    message: Message,
) -> Element<'static, Message> {
    widget::button(
        widget::row![
            widget::text(emoji)
                .size(18)
                .width(Length::Fixed(28.0))
                .align_x(Horizontal::Center),
            widget::text(label)
                .size(13)
                .color(text_main())
                .width(Length::Fill),
            widget::text("↗").size(14).color(accent_2()),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([13, 15])
    .width(Length::Fill)
    .height(Length::Fixed(58.0))
    .style(|_theme, status| {
        let background = match status {
            widget::button::Status::Hovered => suggestion_hover_background(),
            _ => panel_soft(),
        };
        let border = if matches!(status, widget::button::Status::Hovered) {
            accent()
        } else {
            border_soft()
        };
        button_visual(background, border, text_main(), status)
    })
    .on_press(message)
    .into()
}

pub(super) fn suggestion_grid(
    labels: [&'static str; 4],
    prompts: [&'static str; 4],
) -> Element<'static, Message> {
    const EMOJIS: [&str; 4] = ["🧠", "🗺️", "💻", "💡"];

    widget::responsive(move |size| {
        let suggestion = |index: usize| {
            suggestion_button(
                EMOJIS[index],
                labels[index],
                Message::UseSuggestion(prompts[index].to_string()),
            )
        };

        if size.width < 480.0 {
            widget::column![suggestion(0), suggestion(1), suggestion(2), suggestion(3)]
                .spacing(iced::Pixels(10.0))
                .width(Length::Fill)
                .into()
        } else {
            widget::column![
                widget::row![suggestion(0), suggestion(1)]
                    .spacing(iced::Pixels(10.0))
                    .width(Length::Fill),
                widget::row![suggestion(2), suggestion(3)]
                    .spacing(iced::Pixels(10.0))
                    .width(Length::Fill),
            ]
            .spacing(iced::Pixels(10.0))
            .width(Length::Fill)
            .into()
        }
    })
    .height(Length::Shrink)
    .into()
}

pub(super) fn send_button<'a>(message: Option<Message>) -> Element<'a, Message> {
    widget::button(
        container(widget::text("↑").size(20).align_x(Horizontal::Center))
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fixed(44.0))
    .height(Length::Fixed(44.0))
    .padding(0)
    .style(|_theme, status| {
        let (background, border) = primary_button_colors();
        button_visual(background, border, Color::WHITE, status)
    })
    .on_press_maybe(message)
    .into()
}

pub(super) fn assistant_mark_style(pulse: f32, active: bool) -> impl Fn(&Theme) -> Style {
    move |_theme: &Theme| Style {
        snap: true,
        text_color: Some(Color::WHITE),
        background: Some(Background::Color(brighten(
            assistant_mark_background(),
            if active { pulse * 0.018 } else { 0.0 },
        ))),
        border: Border {
            color: brighten(accent(), if active { pulse * 0.018 } else { 0.0 }),
            width: if active { 1.0 + pulse * 0.18 } else { 1.0 },
            radius: Radius::from(14.0),
        },
        shadow: Shadow {
            color: with_alpha(accent(), if active { 0.20 + pulse * 0.08 } else { 0.20 }),
            offset: Vector::from([0.0, if active { 4.0 + pulse } else { 4.0 }]),
            blur_radius: if active { 14.0 + pulse * 4.0 } else { 16.0 },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{InterfaceTheme, control_accent_for, rgb};

    #[test]
    fn only_dark_uses_blue_for_the_control_accent() {
        assert_eq!(control_accent_for(InterfaceTheme::Dark), rgb(82, 140, 255));
        assert_eq!(control_accent_for(InterfaceTheme::Light), rgb(87, 214, 198));
        assert_eq!(
            control_accent_for(InterfaceTheme::Modern),
            rgb(87, 214, 198)
        );
    }
}
