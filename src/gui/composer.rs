use iced::keyboard::{Key, key};
use iced::widget::text_editor::{Binding, KeyPress, Status};

use crate::Message;

pub(super) fn prompt_key_binding(press: KeyPress, prompt: &str) -> Option<Binding<Message>> {
    if !matches!(press.status, Status::Focused { .. }) {
        return None;
    }
    if press.key == Key::Named(key::Named::Enter) && !press.modifiers.shift() {
        return Some(Binding::Custom(Message::Prompt(prompt.to_owned())));
    }
    let paste_shortcut = press.modifiers.command()
        && !press.modifiers.alt()
        && press
            .key
            .to_latin(press.physical_key)
            .is_some_and(|key| key.eq_ignore_ascii_case(&'v'));
    let shift_insert = press.key == Key::Named(key::Named::Insert)
        && press.modifiers == iced::keyboard::Modifiers::SHIFT;
    // Resolve image/text paste from the focused composer. A global ignored-event
    // subscription never sees paste keys already captured by the editor.
    if paste_shortcut || shift_insert {
        Some(Binding::Custom(Message::PastePrompt))
    } else {
        Binding::from_key_press(press)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::Modifiers;

    fn press(key: Key, code: key::Code, modifiers: Modifiers, status: Status) -> KeyPress {
        KeyPress {
            modified_key: key.clone(),
            key,
            physical_key: key::Physical::Code(code),
            modifiers,
            text: None,
            status,
        }
    }

    #[test]
    fn paste_routes_through_composer_for_latin_and_non_latin_layouts() {
        for letter in ["v", "V", "м"] {
            assert!(matches!(
                prompt_key_binding(
                    press(
                        Key::Character(letter.into()),
                        key::Code::KeyV,
                        Modifiers::COMMAND,
                        Status::Focused { is_hovered: false },
                    ),
                    "draft"
                ),
                Some(Binding::Custom(Message::PastePrompt))
            ));
        }
    }

    #[test]
    fn unfocused_composer_cannot_paste_or_submit() {
        for status in [Status::Active, Status::Hovered, Status::Disabled] {
            for (key, code, modifiers) in [
                (
                    Key::Character("v".into()),
                    key::Code::KeyV,
                    Modifiers::COMMAND,
                ),
                (
                    Key::Named(key::Named::Enter),
                    key::Code::Enter,
                    Modifiers::empty(),
                ),
            ] {
                assert!(prompt_key_binding(press(key, code, modifiers, status), "draft").is_none());
            }
        }
    }

    #[test]
    fn shift_insert_pastes_but_altgr_v_does_not() {
        let focused = Status::Focused { is_hovered: false };
        assert!(matches!(
            prompt_key_binding(
                press(
                    Key::Named(key::Named::Insert),
                    key::Code::Insert,
                    Modifiers::SHIFT,
                    focused,
                ),
                ""
            ),
            Some(Binding::Custom(Message::PastePrompt))
        ));
        assert!(!matches!(
            prompt_key_binding(
                press(
                    Key::Character("v".into()),
                    key::Code::KeyV,
                    Modifiers::CTRL | Modifiers::ALT,
                    focused,
                ),
                ""
            ),
            Some(Binding::Custom(Message::PastePrompt))
        ));
    }

    #[test]
    fn enter_submits_shift_enter_inserts_a_line_and_copy_still_works() {
        let focused = Status::Focused { is_hovered: true };
        assert!(matches!(prompt_key_binding(press(
            Key::Named(key::Named::Enter), key::Code::Enter, Modifiers::empty(), focused,
        ), "draft"), Some(Binding::Custom(Message::Prompt(text))) if text == "draft"));
        assert!(matches!(
            prompt_key_binding(
                press(
                    Key::Named(key::Named::Enter),
                    key::Code::Enter,
                    Modifiers::SHIFT,
                    focused,
                ),
                "draft"
            ),
            Some(Binding::Enter)
        ));
        assert!(matches!(
            prompt_key_binding(
                press(
                    Key::Character("c".into()),
                    key::Code::KeyC,
                    Modifiers::COMMAND,
                    focused,
                ),
                "draft"
            ),
            Some(Binding::Copy)
        ));
    }
}
