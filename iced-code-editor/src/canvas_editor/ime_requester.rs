use iced::advanced::input_method;
use iced::advanced::widget::{Widget, tree};
use iced::advanced::{Renderer, Shell};
use iced::{Event, Length, Rectangle, Size, Vector, mouse, window};

#[derive(Debug, Clone)]
pub struct ImeRequester {
    // -------------------------------------------------------------------------
    // IME requester state fields
    // -------------------------------------------------------------------------

    // Whether IME is enabled
    // Logic: true only when the editor has both Iced focus (is_focused) and
    // internal canvas focus (has_canvas_focus). This maps to the
    // Enabled/Disabled state of `shell.request_input_method`.
    enabled: bool,

    // IME caret rectangle
    // Purpose: tells the OS the exact caret location on screen (x, y, w, h).
    // The OS uses this to position the candidate window near the caret and
    // avoid covering it (the "over-the-spot" style).
    cursor: Rectangle,

    // Current preedit content
    // Purpose: send current preedit text (e.g. "nihao") back to the Shell.
    // Although the Shell usually sends it to the View, we keep it here to keep
    // requests consistent.
    preedit: Option<input_method::Preedit<String>>,
}

impl ImeRequester {
    /// Creates a new IME requester widget.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether the IME interaction is enabled (usually true when editor is focused).
    /// * `cursor` - The visual cursor position and size relative to the editor content.
    /// * `preedit` - The current pre-edit text state, if any.
    pub fn new(
        enabled: bool,
        cursor: Rectangle,
        preedit: Option<input_method::Preedit<String>>,
    ) -> Self {
        Self { enabled, cursor, preedit }
    }
}

// The ImeRequester widget implements a size of Length::Shrink for both dimensions
// but returns a zero-size layout. This creates an invisible widget that only exists
// to call shell.request_input_method. Consider documenting this design pattern more
// explicitly in the struct-level documentation, as it's a non-standard use of the
// Widget trait where the widget serves as a bridge rather than a visual element.
impl<Message> Widget<Message, iced::Theme, iced::Renderer> for ImeRequester
where
    iced::Renderer: Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        _tree: &mut tree::Tree,
        _renderer: &iced::Renderer,
        _limits: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        iced::advanced::layout::Node::new(Size::new(0.0, 0.0))
    }

    fn draw(
        &self,
        _tree: &tree::Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &iced::advanced::renderer::Style,
        _layout: iced::advanced::layout::Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn state(&self) -> tree::State {
        tree::State::None
    }

    fn update(
        &mut self,
        _tree: &mut tree::Tree,
        event: &Event,
        layout: iced::advanced::layout::Layout<'_>,
        _cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        // Core IME request logic
        // ---------------------------------------------------------------------
        // Why request on `RedrawRequested`?
        // 1. Iced's IME protocol requires explicit IME state each frame or on changes.
        // 2. `RedrawRequested` starts the render cycle, ensuring the OS gets the
        //    latest caret position so the candidate window tracks movement.
        //
        // NOTE: While it might seem beneficial to request IME updates on input events
        // (like KeyPressed) for lower latency, doing so would use STALE cursor
        // positions from the previous frame (because the widget hasn't been rebuilt
        // with the new state yet). `RedrawRequested` guarantees we are using the
        // fresh cursor position calculated in the latest `view()` pass.
        //
        // Branches:
        // - enabled = true: editor active and focused. Request `InputMethod::Enabled`
        //   with the caret rectangle (cursor) and preedit content (preedit).
        // - enabled = false: editor unfocused. Request `InputMethod::Disabled`
        //   to close the soft keyboard or reset IME state.
        // ---------------------------------------------------------------------
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            if self.enabled {
                // Get the absolute position of the widget in the window
                // This is required because the OS IME API expects window-relative coordinates
                // not widget-relative ones. Without this, the candidate window would
                // appear at the top-left of the window instead of near the cursor.
                let position = layout.bounds().position();
                let cursor_rect = Rectangle {
                    x: self.cursor.x + position.x,
                    y: self.cursor.y + position.y,
                    width: self.cursor.width,
                    height: self.cursor.height,
                };

                let ime = input_method::InputMethod::Enabled {
                    cursor: cursor_rect,
                    purpose: input_method::Purpose::Normal,
                    preedit: self
                        .preedit
                        .as_ref()
                        .map(input_method::Preedit::as_ref),
                };
                shell.request_input_method(&ime);
            } else {
                let disabled: input_method::InputMethod<&str> =
                    input_method::InputMethod::Disabled;
                shell.request_input_method(&disabled);
            }
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &tree::Tree,
        _layout: iced::advanced::layout::Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        mouse::Interaction::None
    }

    fn overlay<'a>(
        &'a mut self,
        _tree: &'a mut tree::Tree,
        _layout: iced::advanced::layout::Layout<'a>,
        _renderer: &iced::Renderer,
        _viewport: &Rectangle,
        _translation: Vector,
    ) -> Option<iced::overlay::Element<'a, Message, iced::Theme, iced::Renderer>>
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::{Length, Point, Size};

    /// Tests the initialization of ImeRequester.
    ///
    /// Verifies that:
    /// 1. The enabled state is correctly stored.
    /// 2. The cursor rectangle is preserved.
    /// 3. The preedit content is correctly passed through.
    #[test]
    fn test_ime_requester_initialization() {
        // Setup test data
        let cursor =
            Rectangle::new(Point::new(10.0, 10.0), Size::new(2.0, 20.0));
        let preedit = Some(input_method::Preedit {
            content: "test".to_string(),
            selection: None,
            text_size: None,
        });

        // Create instance
        let requester = ImeRequester::new(true, cursor, preedit.clone());

        // Assertions
        assert!(requester.enabled, "Should be enabled");
        assert_eq!(requester.cursor, cursor, "Cursor rect should match");

        // Verify preedit content matches
        if let Some(p) = requester.preedit {
            assert_eq!(p.content, "test", "Preedit content should match");
        } else {
            panic!("Preedit should be Some");
        }
    }

    /// Tests the Widget trait implementation details.
    ///
    /// Verifies that:
    /// 1. size() returns Shrink/Shrink (invisible widget).
    /// 2. tag() returns stateless tag.
    /// 3. state() returns None (no internal state management needed).
    #[test]
    fn test_ime_requester_layout_properties() {
        let cursor = Rectangle::new(Point::new(0.0, 0.0), Size::new(0.0, 0.0));
        let requester = ImeRequester::new(false, cursor, None);

        // Test size strategy - should be Shrink/Shrink
        let size =
            <ImeRequester as Widget<(), iced::Theme, iced::Renderer>>::size(
                &requester,
            );
        assert_eq!(size.width, Length::Shrink, "Width should be Shrink");
        assert_eq!(size.height, Length::Shrink, "Height should be Shrink");

        // Test widget tag - should be stateless
        assert_eq!(
            <ImeRequester as Widget<(), iced::Theme, iced::Renderer>>::tag(
                &requester
            ),
            tree::Tag::stateless(),
            "Widget should be stateless"
        );

        // Test widget state - should be None
        assert!(matches!(
            <ImeRequester as Widget<(), iced::Theme, iced::Renderer>>::state(&requester),
            tree::State::None
        ), "Widget state should be None");
    }
}
