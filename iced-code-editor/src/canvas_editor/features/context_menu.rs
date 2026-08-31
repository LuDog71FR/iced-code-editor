//! Right-click context menu for editor actions.

use iced::widget::{Space, button, column, container, row, text};
use iced::{Background, Border, Color, Element, Length, Shadow, Theme, Vector};

use super::actions::{ActionContext, SharedAction};
use crate::canvas_editor::Message;
use crate::i18n::Translations;

const MENU_WIDTH: f32 = 224.0;

/// An actionable entry in the editor context menu.
///
/// The `id` is what the host receives when the item is selected, so it should
/// be a stable identifier rather than the display text — renaming or
/// translating a `label` then never breaks the action.
///
/// # Examples
///
/// ```
/// use iced_code_editor::ContextMenuItem;
///
/// let item = ContextMenuItem::new("format", "Format Document")
///     .with_shortcut("Ctrl+Shift+F");
///
/// assert_eq!(item.id, "format");
/// assert_eq!(item.shortcut.as_deref(), Some("Ctrl+Shift+F"));
/// assert!(item.enabled);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextMenuItem {
    /// Stable action identifier emitted when the item is selected.
    pub id: String,
    /// Text displayed for the item.
    pub label: String,
    /// Optional keyboard shortcut hint displayed beside the label.
    pub shortcut: Option<String>,
    /// Whether the item can be selected.
    pub enabled: bool,
    /// Current state of a toggle command, `None` for every other command.
    ///
    /// Only the command palette reads it, where it is drawn as an On/Off badge
    /// beside the label; the context menu ignores it. Set it on an entry that
    /// switches a setting the host owns, and re-register the entries when that
    /// setting changes so the badge stays truthful.
    pub status: Option<bool>,
}

impl ContextMenuItem {
    /// Creates an enabled context-menu item without a shortcut hint.
    ///
    /// # Arguments
    ///
    /// * `id` - Stable action identifier emitted when the item is selected
    /// * `label` - The text shown to the user
    ///
    /// # Returns
    ///
    /// An enabled item with no shortcut hint
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::ContextMenuItem;
    ///
    /// let item = ContextMenuItem::new("rename", "Rename Symbol");
    /// assert_eq!(item.label, "Rename Symbol");
    /// assert!(item.shortcut.is_none());
    /// assert!(item.enabled);
    /// ```
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            shortcut: None,
            enabled: true,
            status: None,
        }
    }

    /// Sets the keyboard shortcut hint displayed beside this item.
    ///
    /// This is only a hint for the user: setting it does not bind the
    /// shortcut, which the host application is responsible for handling.
    ///
    /// # Arguments
    ///
    /// * `shortcut` - The shortcut text to display, e.g. `Ctrl+Shift+F`
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::ContextMenuItem;
    ///
    /// let item = ContextMenuItem::new("format", "Format Document")
    ///     .with_shortcut("Ctrl+Shift+F");
    /// assert_eq!(item.shortcut.as_deref(), Some("Ctrl+Shift+F"));
    /// ```
    #[must_use]
    pub fn with_shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }

    /// Sets whether this item can be selected.
    ///
    /// A disabled item is still drawn, dimmed — use this for an action that is
    /// unavailable right now but should stay visible so the menu does not
    /// change shape between right-clicks.
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether the item can be selected
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::ContextMenuItem;
    ///
    /// let item = ContextMenuItem::new("rename", "Rename Symbol")
    ///     .with_enabled(false);
    /// assert!(!item.enabled);
    /// ```
    #[must_use]
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Marks this item as a toggle currently in state `status`.
    ///
    /// The command palette then shows an On/Off badge beside the label, so the
    /// user can tell what running the command will do without trying it. The
    /// value is a snapshot: re-register the entries when the setting changes.
    ///
    /// # Arguments
    ///
    /// * `status` - Whether the setting the item toggles is currently on
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::ContextMenuItem;
    ///
    /// let item = ContextMenuItem::new("format_on_save", "Toggle Format On Save")
    ///     .with_status(true);
    /// assert_eq!(item.status, Some(true));
    /// ```
    #[must_use]
    pub fn with_status(mut self, status: bool) -> Self {
        self.status = Some(status);
        self
    }
}

/// A custom editor context-menu entry.
///
/// Pass a list of these to [`CodeEditor::set_custom_context_menu_entries`] to
/// extend the menu, or to replace it entirely by also turning off the built-in
/// actions with [`CodeEditor::set_default_context_menu_enabled`].
///
/// # Examples
///
/// ```
/// use iced_code_editor::ContextMenuEntry;
///
/// let entries = vec![
///     ContextMenuEntry::item("format", "Format Document")
///         .with_shortcut("Ctrl+Shift+F"),
///     ContextMenuEntry::separator(),
///     ContextMenuEntry::item("rename", "Rename Symbol").with_enabled(false),
/// ];
/// assert_eq!(entries.len(), 3);
/// ```
///
/// [`CodeEditor::set_custom_context_menu_entries`]: crate::CodeEditor::set_custom_context_menu_entries
/// [`CodeEditor::set_default_context_menu_enabled`]: crate::CodeEditor::set_default_context_menu_enabled
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextMenuEntry {
    /// An actionable menu item.
    Item(ContextMenuItem),
    /// A visual separator between groups of items.
    Separator,
}

impl ContextMenuEntry {
    /// Creates an enabled action entry without a shortcut hint.
    ///
    /// Shorthand for wrapping [`ContextMenuItem::new`].
    ///
    /// # Arguments
    ///
    /// * `id` - Stable action identifier emitted when the entry is selected
    /// * `label` - The text shown to the user
    ///
    /// # Returns
    ///
    /// An [`ContextMenuEntry::Item`] wrapping a new enabled item
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{ContextMenuEntry, ContextMenuItem};
    ///
    /// let entry = ContextMenuEntry::item("format", "Format Document");
    /// assert_eq!(
    ///     entry,
    ///     ContextMenuEntry::Item(ContextMenuItem::new("format", "Format Document")),
    /// );
    /// ```
    pub fn item(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Item(ContextMenuItem::new(id, label))
    }

    /// Creates a separator entry.
    ///
    /// # Returns
    ///
    /// A [`ContextMenuEntry::Separator`]
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::ContextMenuEntry;
    ///
    /// assert_eq!(ContextMenuEntry::separator(), ContextMenuEntry::Separator);
    /// ```
    pub const fn separator() -> Self {
        Self::Separator
    }

    /// Sets the keyboard shortcut hint when this is an action entry.
    ///
    /// A separator has no shortcut to set, so it is returned unchanged rather
    /// than erroring — this keeps a chain of builder calls over a mixed list
    /// of entries free of special cases.
    ///
    /// # Arguments
    ///
    /// * `shortcut` - The shortcut text to display, e.g. `Ctrl+Shift+F`
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::ContextMenuEntry;
    ///
    /// let entry = ContextMenuEntry::item("format", "Format Document")
    ///     .with_shortcut("Ctrl+Shift+F");
    /// assert!(matches!(entry, ContextMenuEntry::Item(_)));
    ///
    /// // A separator is unaffected.
    /// let separator = ContextMenuEntry::separator().with_shortcut("Ctrl+K");
    /// assert_eq!(separator, ContextMenuEntry::Separator);
    /// ```
    #[must_use]
    pub fn with_shortcut(self, shortcut: impl Into<String>) -> Self {
        match self {
            Self::Item(item) => Self::Item(item.with_shortcut(shortcut)),
            Self::Separator => Self::Separator,
        }
    }

    /// Sets whether this entry can be selected when it is an action.
    ///
    /// A separator is returned unchanged, for the same reason as
    /// [`Self::with_shortcut`].
    ///
    /// # Arguments
    ///
    /// * `enabled` - Whether the entry can be selected
    ///
    /// # Returns
    ///
    /// Self for method chaining
    ///
    /// # Examples
    ///
    /// ```
    /// use iced_code_editor::{ContextMenuEntry, ContextMenuItem};
    ///
    /// let entry = ContextMenuEntry::item("rename", "Rename Symbol")
    ///     .with_enabled(false);
    /// assert_eq!(
    ///     entry,
    ///     ContextMenuEntry::Item(
    ///         ContextMenuItem::new("rename", "Rename Symbol").with_enabled(false)
    ///     ),
    /// );
    ///
    /// // A separator is unaffected.
    /// let separator = ContextMenuEntry::separator().with_enabled(false);
    /// assert_eq!(separator, ContextMenuEntry::Separator);
    /// ```
    #[must_use]
    pub fn with_enabled(self, enabled: bool) -> Self {
        match self {
            Self::Item(item) => Self::Item(item.with_enabled(enabled)),
            Self::Separator => Self::Separator,
        }
    }
}

impl From<ContextMenuItem> for ContextMenuEntry {
    fn from(item: ContextMenuItem) -> Self {
        Self::Item(item)
    }
}

#[derive(Debug, Clone)]
enum MenuEntry {
    Item { label: String, shortcut: String, message: Option<Message> },
    Separator,
}

impl MenuEntry {
    #[cfg(test)]
    fn label(&self) -> Option<&str> {
        match self {
            Self::Item { label, .. } => Some(label),
            Self::Separator => None,
        }
    }
}

fn custom_entries(entries: &[ContextMenuEntry]) -> Vec<MenuEntry> {
    entries
        .iter()
        .map(|entry| match entry {
            ContextMenuEntry::Item(item) => MenuEntry::Item {
                label: item.label.clone(),
                shortcut: item.shortcut.clone().unwrap_or_default(),
                message: item
                    .enabled
                    .then(|| Message::CustomContextMenuAction(item.id.clone())),
            },
            ContextMenuEntry::Separator => MenuEntry::Separator,
        })
        .collect()
}

/// Builds the built-in half of the menu, in the order the menu shows it.
///
/// Every row's label, shortcut and message come from [`SharedAction`], which
/// the command palette reads too; what belongs to the menu is the order, the
/// separators, and the choice to *dim* an unavailable action rather than drop
/// it. A menu with a fixed shape is what lets muscle memory work, so a row
/// that cannot be run right now still holds its place.
fn default_entries(
    context: ActionContext,
    translations: &Translations,
) -> Vec<MenuEntry> {
    /// Turns a shared action into a menu row, dimmed when unavailable.
    fn row(
        action: SharedAction,
        context: ActionContext,
        translations: &Translations,
    ) -> MenuEntry {
        MenuEntry::Item {
            label: action.label(translations),
            shortcut: action.shortcut().to_string(),
            message: action.is_available(context).then(|| action.message()),
        }
    }

    // Reveal sits above everything, separated: it acts on the file rather than
    // on the text, and it is the only one the host can switch off entirely --
    // so it is absent, not dimmed, when unavailable.
    let mut entries = if SharedAction::RevealInFileManager.is_available(context)
    {
        vec![
            row(SharedAction::RevealInFileManager, context, translations),
            MenuEntry::Separator,
        ]
    } else {
        Vec::new()
    };

    entries.extend([
        row(SharedAction::Undo, context, translations),
        row(SharedAction::Redo, context, translations),
        MenuEntry::Separator,
        row(SharedAction::Cut, context, translations),
        row(SharedAction::Copy, context, translations),
        row(SharedAction::Paste, context, translations),
        MenuEntry::Separator,
        row(SharedAction::SelectAll, context, translations),
    ]);
    entries
}

fn build_entries(
    custom: &[ContextMenuEntry],
    default_context_menu_enabled: bool,
    context: ActionContext,
    translations: &Translations,
) -> Vec<MenuEntry> {
    let mut entries = custom_entries(custom);
    if default_context_menu_enabled {
        if !entries.is_empty() {
            entries.push(MenuEntry::Separator);
        }
        entries.extend(default_entries(context, translations));
    }
    entries
}

/// Builds the context-menu contents.
pub(crate) fn view(
    custom: &[ContextMenuEntry],
    default_context_menu_enabled: bool,
    context: ActionContext,
    translations: Translations,
) -> Element<'static, Message> {
    let items = build_entries(
        custom,
        default_context_menu_enabled,
        context,
        &translations,
    )
    .into_iter()
    .map(|entry| match entry {
        MenuEntry::Item { label, shortcut, message } => {
            menu_item(label, shortcut, message)
        }
        MenuEntry::Separator => separator(),
    })
    .collect::<Vec<_>>();

    container(column(items).spacing(1).padding(4))
        .width(Length::Fixed(MENU_WIDTH))
        .style(|theme: &Theme| {
            let palette = theme.extended_palette();
            container::Style {
                background: Some(Background::Color(
                    palette.background.weak.color,
                )),
                text_color: Some(palette.background.weak.text),
                border: Border {
                    color: palette.background.strong.color,
                    width: 1.0,
                    radius: 6.0.into(),
                },
                shadow: Shadow {
                    color: Color::BLACK.scale_alpha(0.35),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 14.0,
                },
                ..container::Style::default()
            }
        })
        .into()
}

fn menu_item(
    label: String,
    shortcut: String,
    message: Option<Message>,
) -> Element<'static, Message> {
    let enabled = message.is_some();
    let content = row![
        text(label).size(13),
        Space::new().width(Length::Fill),
        text(shortcut).size(12),
    ]
    .align_y(iced::Alignment::Center);

    button(content)
        .width(Length::Fill)
        .padding([6, 9])
        .on_press_maybe(message)
        .style(move |theme: &Theme, status| {
            let palette = theme.extended_palette();
            let text_color = if enabled {
                palette.background.weak.text
            } else {
                palette.background.weak.text.scale_alpha(0.35)
            };
            let background = matches!(
                status,
                button::Status::Hovered | button::Status::Pressed
            )
            .then_some(Background::Color(palette.background.strong.color));

            button::Style {
                background,
                text_color,
                border: Border { radius: 4.0.into(), ..Border::default() },
                ..button::Style::default()
            }
        })
        .into()
}

fn separator() -> Element<'static, Message> {
    let line =
        container(Space::new().width(Length::Fill).height(Length::Fixed(1.0)))
            .style(|theme: &Theme| {
                let palette = theme.extended_palette();
                container::Style {
                    background: Some(Background::Color(
                        palette.background.strong.color,
                    )),
                    ..container::Style::default()
                }
            });

    container(line).padding([3, 7]).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::canvas_editor::CodeEditor;
    use crate::canvas_editor::features::actions::SharedAction;
    use crate::canvas_editor::features::command_palette::PaletteAction;
    use crate::{ContextMenuEntry, ContextMenuItem, Language, Translations};

    /// An editor in which every shared action is available, so both surfaces
    /// offer all seven and the two lists can be compared row for row.
    fn editor_with_everything_available() -> CodeEditor {
        let mut editor = CodeEditor::new("hello", "rs");
        editor.set_reveal_in_file_manager_enabled(true);
        // Paste rather than type: typed characters stay in an open undo
        // group, so nothing would be undoable yet. Two edits and one undo
        // leaves both stacks non-empty at once, which is what makes Undo and
        // Redo simultaneously available.
        let _ = editor.update(&Message::Paste("a".to_string()));
        let _ = editor.update(&Message::Paste("b".to_string()));
        let _ = editor.update(&Message::Undo);
        let _ = editor.update(&Message::SelectAll);

        let context = editor.action_context();
        assert!(context.can_undo && context.can_redo);
        assert!(context.has_selection && context.has_content);
        assert!(context.reveal_in_file_manager_enabled);

        editor
    }

    // A missing row or a mismatched binding *is* the failure this test
    // reports, so `panic!` is the report rather than a bug — the same reason
    // the LSP protocol tests carry this allow.
    #[test]
    #[allow(clippy::panic)]
    fn test_both_surfaces_render_every_shared_action_from_the_same_binding() {
        // The drift this exists to prevent: the two surfaces used to spell out
        // label / shortcut / message separately, so a relabelled action or a
        // rebound key had to be edited in both, and nothing noticed when only
        // one was. Reading them from `SharedAction` is what makes them equal;
        // this walks all seven and checks that they really are.
        let editor = editor_with_everything_available();
        let translations = Translations::default();
        let menu =
            build_entries(&[], true, editor.action_context(), &translations);
        let palette = editor.command_palette_entries();

        for action in SharedAction::ALL {
            let label = action.label(&translations);

            let menu_row = menu
                .iter()
                .find_map(|entry| match entry {
                    MenuEntry::Item { label: found, shortcut, message }
                        if *found == label =>
                    {
                        Some((shortcut, message))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{label:?} missing from the menu"));

            let palette_row = palette
                .iter()
                .find(|entry| entry.label == label)
                .unwrap_or_else(|| {
                    panic!("{label:?} missing from the palette")
                });

            assert_eq!(
                menu_row.0, &palette_row.shortcut,
                "{label:?}: the two surfaces show different shortcut hints"
            );
            assert_eq!(
                menu_row.0,
                action.shortcut(),
                "{label:?}: neither matches `SharedAction::shortcut`"
            );

            // `Message` is not `PartialEq`; its `Debug` form is enough to tell
            // two variants -- and two `Paste` payloads -- apart.
            let expected = format!("{:?}", action.message());
            assert_eq!(
                menu_row.1.as_ref().map(|message| format!("{message:?}")),
                Some(expected.clone()),
                "{label:?}: the menu sends a different message"
            );
            let sent = match &palette_row.action {
                PaletteAction::Builtin(message) => format!("{message:?}"),
                other => panic!("{label:?} is not a built-in row: {other:?}"),
            };
            assert_eq!(
                sent, expected,
                "{label:?}: the palette sends a different message"
            );
        }
    }

    #[test]
    fn test_an_unavailable_action_is_dimmed_in_the_menu_but_absent_from_the_palette()
     {
        // The one thing the two surfaces are *meant* to disagree on. A fresh
        // editor has nothing to undo, and no file behind it to reveal.
        let editor = CodeEditor::new("hello", "rs");
        let translations = Translations::default();
        let undo = SharedAction::Undo.label(&translations);
        let reveal = SharedAction::RevealInFileManager.label(&translations);

        let menu =
            build_entries(&[], true, editor.action_context(), &translations);
        let palette = editor.command_palette_entries();

        assert!(
            menu.iter().any(|entry| matches!(
                entry,
                MenuEntry::Item { label, message: None, .. } if *label == undo
            )),
            "the menu keeps Undo in place, dimmed"
        );
        assert!(
            !palette.iter().any(|entry| entry.label == undo),
            "the palette drops Undo entirely"
        );

        // Reveal is the exception: the host switched it off, so it is absent
        // from both rather than dimmed in one.
        assert!(!menu.iter().any(|entry| entry.label() == Some(&reveal)));
        assert!(!palette.iter().any(|entry| entry.label == reveal));
    }

    #[test]
    fn test_custom_context_menu_action_message_preserves_id() {
        let entries = build_entries(
            &[ContextMenuEntry::Item(ContextMenuItem::new(
                "refactor.extract",
                "Extract function",
            ))],
            false,
            ActionContext::default(),
            &Translations::default(),
        );

        assert!(matches!(
            &entries[0],
            MenuEntry::Item {
                message: Some(Message::CustomContextMenuAction(id)),
                ..
            }
                if id == "refactor.extract"
        ));
    }

    #[test]
    fn test_custom_entries_precede_default_entries() {
        let entries = build_entries(
            &[ContextMenuEntry::item("custom.format", "Format document")],
            true,
            ActionContext::default(),
            &Translations::default(),
        );

        assert_eq!(entries[0].label(), Some("Format document"));
        assert!(matches!(entries[1], MenuEntry::Separator));
        assert_eq!(entries[2].label(), Some("Undo"));
    }

    #[test]
    fn test_context_menu_uses_selected_language() {
        let translations = Translations::new(Language::ChineseSimplified);
        let entries = default_entries(ActionContext::default(), &translations);

        assert_eq!(entries[0].label(), Some("撤消"));
        assert_eq!(entries[1].label(), Some("恢复"));
        assert_eq!(entries[3].label(), Some("剪切"));
        assert_eq!(entries[4].label(), Some("复制"));
        assert_eq!(entries[5].label(), Some("粘贴"));
        assert_eq!(entries[7].label(), Some("选择全部"));

        let custom = custom_entries(&[ContextMenuEntry::item(
            "custom.format",
            "Format document",
        )]);
        assert_eq!(custom[0].label(), Some("Format document"));
    }

    #[test]
    fn test_reveal_in_file_manager_entry_emits_request() {
        let translations = Translations::new(Language::English);
        let entries = build_entries(
            &[],
            true,
            ActionContext {
                reveal_in_file_manager_enabled: true,
                ..ActionContext::default()
            },
            &translations,
        );

        assert!(matches!(
            &entries[0],
            MenuEntry::Item { label, shortcut, message }
                if label == &translations.context_menu_reveal_in_file_manager()
                    && shortcut.is_empty()
                    && matches!(message, Some(Message::RevealInFileManager))
        ));
        assert!(matches!(entries[1], MenuEntry::Separator));
        assert_eq!(entries[2].label(), Some("Undo"));
    }

    #[test]
    fn test_reveal_in_file_manager_respects_default_menu_toggle() {
        let entries = build_entries(
            &[],
            false,
            ActionContext {
                reveal_in_file_manager_enabled: true,
                ..ActionContext::default()
            },
            &Translations::default(),
        );

        assert!(entries.is_empty());
    }
}
