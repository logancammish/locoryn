use iced::advanced::widget::{Operation, Tree, operation::Focusable, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector, keyboard, mouse};

use crate::Message;

pub(super) fn conversation_shortcuts<'a>(
    content: impl Into<Element<'a, Message>>,
    selected: bool,
) -> Element<'a, Message> {
    Element::new(ConversationShortcuts {
        content: content.into(),
        selected,
    })
}

struct ConversationShortcuts<'a, R = Renderer> {
    content: Element<'a, Message, Theme, R>,
    selected: bool,
}

#[derive(Default)]
struct FocusedInput(bool);

impl Operation for FocusedInput {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn focusable(
        &mut self,
        _id: Option<&iced::widget::Id>,
        _bounds: Rectangle,
        state: &mut dyn Focusable,
    ) {
        self.0 |= state.is_focused();
    }
}

fn shortcut(event: &Event, selected: bool, input_focused: bool) -> Option<Message> {
    if input_focused {
        return None;
    }
    match event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            key,
            physical_key,
            modifiers,
            ..
        }) if (modifiers.command() || modifiers.control())
            && !modifiers.alt()
            && !modifiers.shift() =>
        {
            let is_key = |letter: &str, code| {
                matches!(key, keyboard::Key::Character(key) if key.eq_ignore_ascii_case(letter))
                    || *physical_key == code
            };
            if is_key("a", keyboard::key::Code::KeyA) {
                Some(Message::SelectConversation)
            } else if selected && is_key("c", keyboard::key::Code::KeyC) {
                Some(Message::CopyChatTranscript)
            } else {
                None
            }
        }
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        }) if selected => Some(Message::ClearConversationSelection),
        _ => None,
    }
}

// Intercept whole-conversation shortcuts before paragraph-level selection sees
// them. Delegate the complete widget interface so scrolling, overlays and native
// text-input selection continue to work normally.
impl<Renderer: renderer::Renderer> Widget<Message, Theme, Renderer>
    for ConversationShortcuts<'_, Renderer>
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }
    fn tag(&self) -> tree::Tag {
        self.content.as_widget().tag()
    }
    fn state(&self) -> tree::State {
        self.content.as_widget().state()
    }
    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }
    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content.as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if let Some(message) = shortcut(event, self.selected, false) {
            let mut focus = FocusedInput::default();
            self.content
                .as_widget_mut()
                .operate(tree, layout, renderer, &mut focus);
            if !focus.0 {
                // Keep rapid key presses in the same event batch in sync before
                // the application processes the published selection message.
                match message {
                    Message::SelectConversation => self.selected = true,
                    Message::ClearConversationSelection => self.selected = false,
                    _ => {}
                }
                shell.publish(message);
                shell.capture_event();
                return;
            }
        }
        if self.selected
            && matches!(
                event,
                Event::Mouse(mouse::Event::ButtonPressed(_))
                    | Event::Touch(iced::touch::Event::FingerPressed { .. })
            )
        {
            self.selected = false;
            shell.publish(Message::ClearConversationSelection);
        }
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(debug_assertions)]
    #[derive(Default)]
    struct TestClipboard(Option<String>);

    #[cfg(debug_assertions)]
    impl Clipboard for TestClipboard {
        fn read(&self, _kind: iced::advanced::clipboard::Kind) -> Option<String> {
            self.0.clone()
        }
        fn write(&mut self, _kind: iced::advanced::clipboard::Kind, text: String) {
            self.0 = Some(text);
        }
    }

    fn key_press(
        key: keyboard::Key,
        code: keyboard::key::Code,
        modifiers: keyboard::Modifiers,
    ) -> Event {
        Event::Keyboard(keyboard::Event::KeyPressed {
            modified_key: key.clone(),
            key,
            physical_key: keyboard::key::Physical::Code(code),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        })
    }

    fn command_key(letter: &str, code: keyboard::key::Code) -> Event {
        key_press(
            keyboard::Key::Character(letter.into()),
            code,
            keyboard::Modifiers::COMMAND,
        )
    }

    #[cfg(debug_assertions)]
    #[test]
    fn transcript_shortcuts_preserve_focused_input_selection_and_copy() {
        for input_focused in [false, true] {
            let input = iced::widget::TextInput::<Message, Theme, ()>::new("Prompt", "draft text")
                .id("composer")
                .on_input(|_| Message::None);
            let mut widget = ConversationShortcuts {
                content: input.into(),
                selected: false,
            };
            let mut tree = Tree::new(&widget as &dyn Widget<Message, Theme, ()>);
            let viewport = Rectangle::with_size(Size::new(600.0, 100.0));
            let node = widget.layout(
                &mut tree,
                &(),
                &layout::Limits::new(Size::ZERO, viewport.size()),
            );
            if input_focused {
                widget.operate(
                    &mut tree,
                    Layout::new(&node),
                    &(),
                    &mut iced::advanced::widget::operation::focusable::focus::<()>(
                        iced::widget::Id::new("composer"),
                    ),
                );
            }
            let mut clipboard = TestClipboard::default();
            let mut messages = Vec::new();
            // Both keys arrive before application messages rebuild the view.
            for event in [
                Event::Keyboard(keyboard::Event::ModifiersChanged(
                    keyboard::Modifiers::COMMAND,
                )),
                command_key("a", keyboard::key::Code::KeyA),
                command_key("c", keyboard::key::Code::KeyC),
            ] {
                widget.update(
                    &mut tree,
                    &event,
                    Layout::new(&node),
                    mouse::Cursor::Unavailable,
                    &(),
                    &mut clipboard,
                    &mut Shell::new(&mut messages),
                    &viewport,
                );
            }
            if input_focused {
                assert!(messages.is_empty());
                assert_eq!(clipboard.0.as_deref(), Some("draft text"));
                assert!(!widget.selected);
            } else {
                assert!(matches!(
                    messages.as_slice(),
                    [Message::SelectConversation, Message::CopyChatTranscript]
                ));
                assert!(clipboard.0.is_none());
                assert!(widget.selected);
                widget.update(
                    &mut tree,
                    &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                    Layout::new(&node),
                    mouse::Cursor::Unavailable,
                    &(),
                    &mut clipboard,
                    &mut Shell::new(&mut messages),
                    &viewport,
                );
                assert!(!widget.selected);
                assert!(matches!(
                    messages.last(),
                    Some(Message::ClearConversationSelection)
                ));
            }
        }
    }

    #[test]
    fn transcript_shortcuts_require_selection_and_leave_other_key_bindings_alone() {
        let copy = command_key("c", keyboard::key::Code::KeyC);
        assert!(shortcut(&copy, false, false).is_none());
        assert!(shortcut(&copy, true, true).is_none());
        assert!(matches!(
            shortcut(&copy, true, false),
            Some(Message::CopyChatTranscript)
        ));
        let escape = key_press(
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::key::Code::Escape,
            keyboard::Modifiers::empty(),
        );
        assert!(matches!(
            shortcut(&escape, true, false),
            Some(Message::ClearConversationSelection)
        ));
        assert!(shortcut(&escape, false, false).is_none());
        assert!(shortcut(&command_key("v", keyboard::key::Code::KeyV), true, false).is_none());
        let plain_a = key_press(
            keyboard::Key::Character("a".into()),
            keyboard::key::Code::KeyA,
            keyboard::Modifiers::empty(),
        );
        assert!(shortcut(&plain_a, false, false).is_none());
        let non_latin_a = command_key("ф", keyboard::key::Code::KeyA);
        assert!(matches!(
            shortcut(&non_latin_a, false, false),
            Some(Message::SelectConversation)
        ));
    }
}
