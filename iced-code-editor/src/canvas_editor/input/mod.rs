//! Input handling: keyboard shortcut recognition, keyboard/mouse/IME event
//! routing, the IME bridge widget, and the
//! [`Message`](crate::canvas_editor::Message) update logic that turns events
//! into editor state changes.

mod events;
pub(crate) mod ime_requester;
mod shortcuts;
mod update;
