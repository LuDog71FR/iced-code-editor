//! Search toolbar bindings and their platform-specific display labels.
use crate::canvas_editor::Message;
use iced::keyboard::{Key, Modifiers, key::Named};

pub(super) fn hint(
    message: &Message,
    multiline: bool,
    mac: bool,
) -> &'static str {
    match (message, mac) {
        (Message::ToggleCaseSensitive, true) => "⌥ ⌘ C",
        (Message::ToggleCaseSensitive, false) => "Alt+C",
        (Message::ToggleWholeWord, true) => "⌥ ⌘ W",
        (Message::ToggleWholeWord, false) => "Alt+W",
        (Message::ToggleSearchRegex, true) => "⌥ ⌘ R",
        (Message::ToggleSearchRegex, false) => "Alt+R",
        (Message::ToggleSearchReplace, true) => "⌘ ⇧ H",
        (Message::ToggleSearchReplace, false) => "Ctrl+H",
        (Message::ToggleSearchMultiline, true) => "⌥ ⌘ M",
        (Message::ToggleSearchMultiline, false) => "Alt+M",
        (Message::SelectAllSearchMatches, true) => "⌥ ↵",
        (Message::SelectAllSearchMatches, false) => "Alt+Enter",
        (Message::FindPrevious, true) => "⌘ ⇧ G",
        (Message::FindPrevious, false) => "Shift+F3",
        (Message::FindNext, true) => "⌘ G",
        (Message::FindNext, false) => "F3",
        (Message::ReplaceNext, _) if !multiline => {
            if mac {
                "↵"
            } else {
                "Enter"
            }
        }
        (Message::ReplaceNext, true) => "⌘ ⇧ ↵",
        (Message::ReplaceNext, false) => "Ctrl+Shift+Enter",
        (Message::ReplaceAll, true) => "⌘ ↵",
        (Message::ReplaceAll, false) => "Ctrl+Enter",
        (Message::CloseSearch, _) => "Esc",
        _ => "",
    }
}

/// Resolves search-only bindings. Call only while the search toolbar is open.
/// Exact modifiers keep clipboard, host navigation, and text editing intact.
pub(crate) fn resolve(
    key: &Key,
    modified: &Key,
    modifiers: Modifiers,
    replace: bool,
    mac: bool,
) -> Option<Message> {
    let command = if mac { Modifiers::LOGO } else { Modifiers::CTRL };
    let option_command =
        if mac { Modifiers::ALT | command } else { Modifiers::ALT };
    let character = |expected: &str| {
        [key, modified].iter().any(|candidate| matches!(candidate, Key::Character(value) if value.eq_ignore_ascii_case(expected)))
    };
    if modifiers == option_command {
        let option = if character("c") {
            Some(Message::ToggleCaseSensitive)
        } else if character("w") {
            Some(Message::ToggleWholeWord)
        } else if character("r") {
            Some(Message::ToggleSearchRegex)
        } else if character("m") {
            Some(Message::ToggleSearchMultiline)
        } else {
            None
        };
        if option.is_some() {
            return option;
        }
    }
    if character("h")
        && modifiers == if mac { command | Modifiers::SHIFT } else { command }
    {
        return Some(Message::ToggleSearchReplace);
    }
    if mac && character("g") {
        if modifiers == command {
            return Some(Message::FindNext);
        }
        if modifiers == command | Modifiers::SHIFT {
            return Some(Message::FindPrevious);
        }
    }
    match key {
        Key::Named(Named::Enter) if modifiers == Modifiers::ALT => {
            Some(Message::SelectAllSearchMatches)
        }
        Key::Named(Named::Enter) if replace && modifiers == command => {
            Some(Message::ReplaceAll)
        }
        Key::Named(Named::Enter)
            if replace && modifiers == command | Modifiers::SHIFT =>
        {
            Some(Message::ReplaceNext)
        }
        Key::Named(Named::F3) if modifiers.is_empty() => {
            Some(Message::FindNext)
        }
        Key::Named(Named::F3) if modifiers == Modifiers::SHIFT => {
            Some(Message::FindPrevious)
        }
        Key::Named(Named::Escape) if modifiers.is_empty() => {
            Some(Message::CloseSearch)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn platform_bindings_and_display_cover_every_button() {
        for mac in [false, true] {
            let command = if mac { Modifiers::LOGO } else { Modifiers::CTRL };
            let options =
                Modifiers::ALT | if mac { command } else { Modifiers::empty() };
            let cases = [
                (
                    Key::Character("c".into()),
                    options,
                    Message::ToggleCaseSensitive,
                ),
                (Key::Character("w".into()), options, Message::ToggleWholeWord),
                (
                    Key::Character("r".into()),
                    options,
                    Message::ToggleSearchRegex,
                ),
                (
                    Key::Character("m".into()),
                    options,
                    Message::ToggleSearchMultiline,
                ),
                (
                    Key::Character("h".into()),
                    command
                        | if mac {
                            Modifiers::SHIFT
                        } else {
                            Modifiers::empty()
                        },
                    Message::ToggleSearchReplace,
                ),
                (
                    Key::Named(Named::Enter),
                    Modifiers::ALT,
                    Message::SelectAllSearchMatches,
                ),
                (Key::Named(Named::Enter), command, Message::ReplaceAll),
                (
                    Key::Named(Named::Enter),
                    command | Modifiers::SHIFT,
                    Message::ReplaceNext,
                ),
                (Key::Named(Named::F3), Modifiers::empty(), Message::FindNext),
                (
                    Key::Named(Named::F3),
                    Modifiers::SHIFT,
                    Message::FindPrevious,
                ),
                (
                    Key::Named(Named::Escape),
                    Modifiers::empty(),
                    Message::CloseSearch,
                ),
            ];
            for (key, modifiers, expected) in cases {
                assert!(
                    matches!(resolve(&key, &key, modifiers, true, mac), Some(actual) if std::mem::discriminant(&actual) == std::mem::discriminant(&expected)),
                    "{key:?} {modifiers:?} mac={mac}"
                );
                assert!(!hint(&expected, true, mac).is_empty());
            }
            if mac {
                let key = Key::Character("g".into());
                assert!(matches!(
                    resolve(&key, &key, command, true, mac),
                    Some(Message::FindNext)
                ));
                assert!(matches!(
                    resolve(&key, &key, command | Modifiers::SHIFT, true, mac),
                    Some(Message::FindPrevious)
                ));
            }
        }
    }
    #[test]
    fn plain_enter_and_clipboard_are_left_to_the_focused_input() {
        for mac in [false, true] {
            for (key, mods) in [
                (Key::Named(Named::Enter), Modifiers::empty()),
                (
                    Key::Character("c".into()),
                    if mac { Modifiers::LOGO } else { Modifiers::CTRL },
                ),
            ] {
                assert!(resolve(&key, &key, mods, true, mac).is_none());
            }
            let key = Key::Named(Named::Enter);
            let command = if mac { Modifiers::LOGO } else { Modifiers::CTRL };
            assert!(resolve(&key, &key, command, false, mac).is_none());
        }
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;

    #[test]
    fn unexpected_modifiers_do_not_trigger_search_actions() {
        for mac in [false, true] {
            let command = if mac { Modifiers::LOGO } else { Modifiers::CTRL };
            let options =
                Modifiers::ALT | if mac { command } else { Modifiers::empty() };
            for key in [
                Key::Character("c".into()),
                Key::Character("w".into()),
                Key::Character("r".into()),
                Key::Character("m".into()),
            ] {
                for modifiers in
                    [Modifiers::empty(), command, options | Modifiers::SHIFT]
                {
                    assert!(
                        resolve(&key, &key, modifiers, true, mac).is_none(),
                        "{key:?} {modifiers:?} mac={mac}"
                    );
                }
            }
            let enter = Key::Named(Named::Enter);
            assert!(
                resolve(&enter, &enter, command | Modifiers::ALT, true, mac)
                    .is_none()
            );
            assert!(
                resolve(&enter, &enter, command | Modifiers::SHIFT, false, mac)
                    .is_none()
            );
        }
    }

    #[test]
    fn layout_modified_and_uppercase_keys_can_trigger_options() {
        for mac in [false, true] {
            let options = Modifiers::ALT
                | if mac { Modifiers::LOGO } else { Modifiers::empty() };
            assert!(matches!(
                resolve(
                    &Key::Character(";".into()),
                    &Key::Character("R".into()),
                    options,
                    true,
                    mac
                ),
                Some(Message::ToggleSearchRegex)
            ));
            assert!(matches!(
                resolve(
                    &Key::Character("C".into()),
                    &Key::Character("©".into()),
                    options,
                    true,
                    mac
                ),
                Some(Message::ToggleCaseSensitive)
            ));
        }
    }
}
