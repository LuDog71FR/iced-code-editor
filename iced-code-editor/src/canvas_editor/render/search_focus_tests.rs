//! Runs the real tasks against a persistent widget tree so recreating the
//! simulator cannot hide a search field that retained keyboard focus.
use super::*;
use iced::advanced::{
    Layout, Shell, layout,
    renderer::Headless,
    widget::{Operation, Tree, operation::Focusable},
};
use iced::futures::StreamExt;

#[derive(Default)]
struct FocusCount(usize);
impl Operation for FocusCount {
    fn focusable(
        &mut self,
        _: Option<&iced::widget::Id>,
        _: Rectangle,
        state: &mut dyn Focusable,
    ) {
        self.0 += usize::from(state.is_focused());
    }
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }
}

#[test]
fn select_all_unfocuses_live_inputs_without_removing_search_or_replacement_rows()
-> Result<(), &'static str> {
    let renderer = iced::futures::executor::block_on(iced::Renderer::new(
        iced::Font::DEFAULT,
        iced::Pixels(13.0),
        Some("tiny-skia"),
    ))
    .ok_or("renderer unavailable")?;
    for multiline in [false, true] {
        for replacement in [false, true] {
            let mut editor = CodeEditor::new("foo bar foo", "txt");
            let _ = editor.update(&Message::OpenSearchReplace);
            let _ = editor.update(&Message::SearchQueryChanged("foo".into()));
            if multiline {
                let _ = editor.update(&Message::ToggleSearchMultiline);
            }
            let input_id = if replacement {
                editor.search_state.replace_input_id.clone()
            } else {
                editor.search_state.search_input_id.clone()
            };
            let mut view = editor.view();
            let mut tree = Tree::new(&view);
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, Size::new(800.0, 400.0)),
            );
            view.as_widget_mut().operate(
                &mut tree,
                Layout::new(&node),
                &renderer,
                &mut iced::advanced::widget::operation::focusable::focus::<()>(
                    input_id,
                ),
            );
            let mut probe = FocusCount::default();
            view.as_widget_mut().operate(
                &mut tree,
                Layout::new(&node),
                &renderer,
                &mut probe,
            );
            assert_eq!(probe.0, 1);
            drop(view);

            let task = editor.update(&Message::SelectAllSearchMatches);
            assert!(
                editor.search_state.is_open
                    && editor.search_state.is_replace_mode
            );
            assert_eq!(editor.cursors.len(), 2);
            let mut view = editor.view();
            tree.diff(&view);
            let node = view.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, Size::new(800.0, 400.0)),
            );
            let mut stream = iced_runtime::task::into_stream(task)
                .ok_or("focus task missing")?;
            while let Some(action) =
                iced::futures::executor::block_on(stream.next())
            {
                match action {
                    iced_runtime::Action::Widget(mut operation) => {
                        view.as_widget_mut().operate(
                            &mut tree,
                            Layout::new(&node),
                            &renderer,
                            operation.as_mut(),
                        );
                    }
                    iced_runtime::Action::Output(_) => {}
                    _ => return Err("unexpected non-widget action"),
                }
            }
            let mut probe = FocusCount::default();
            view.as_widget_mut().operate(
                &mut tree,
                Layout::new(&node),
                &renderer,
                &mut probe,
            );
            assert_eq!(
                probe.0, 0,
                "retained input focus: multiline={multiline} replacement={replacement}"
            );
            let event =
                iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                    key: iced::keyboard::Key::Character("x".into()),
                    modified_key: iced::keyboard::Key::Character("x".into()),
                    physical_key: iced::keyboard::key::Physical::Unidentified(
                        iced::keyboard::key::NativeCode::Unidentified,
                    ),
                    location: iced::keyboard::Location::Standard,
                    modifiers: iced::keyboard::Modifiers::empty(),
                    text: Some("x".into()),
                    repeat: false,
                });
            let mut messages = Vec::new();
            view.as_widget_mut().update(
                &mut tree,
                &event,
                Layout::new(&node),
                mouse::Cursor::Unavailable,
                &renderer,
                &mut iced::advanced::clipboard::Null,
                &mut Shell::new(&mut messages),
                &node.bounds(),
            );
            assert!(messages.iter().any(|message| matches!(
                message,
                Message::CharacterInput('x')
            )));
            assert!(!messages.iter().any(|message| matches!(
                message,
                Message::SearchQueryChanged(_)
                    | Message::SearchInputAction(_)
                    | Message::ReplaceQueryChanged(_)
                    | Message::ReplaceInputAction(_)
            )));
            drop(view);
            for message in messages {
                let _ = editor.update(&message);
            }
            assert_eq!(editor.content(), "x bar x");
            assert_eq!(editor.search_state.query, "foo");
            assert!(editor.search_state.is_open);
        }
    }
    Ok(())
}
