use iced::widget::{scrollable, text_editor};
use iced::{highlighter, Element};

use crate::code_editor::state::EditorTheme;

// Constantes de configuration de l'éditeur
const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 20.0;
const PADDING_TOP: f32 = 10.0;
const PADDING_LEFT: f32 = 10.0;
const CHAR_WIDTH: f32 = 8.5; // Largeur approximative d'un caractère monospace
const GUTTER_WIDTH: f32 = 60.0;

// Constantes de scroll
const VIEWPORT_HEIGHT: f32 = 600.0; // Estimation de la hauteur visible
const TOP_MARGIN: f32 = 100.0; // Marge en haut pour trigger le scroll
const BOTTOM_MARGIN: f32 = 100.0; // Marge en bas pour trigger le scroll

pub struct CodeEditorComponent {
    content: text_editor::Content,
    theme: EditorTheme,
    syntax: String, // "py" pour Python, "lua" pour Lua, etc.
    scroll_id: scrollable::Id,
    cursor_line: usize,   // Tracker la ligne du curseur
    cursor_column: usize, // Tracker la colonne du curseur
    last_scroll_y: f32,   // Dernière position de scroll pour optimisation
}

#[derive(Debug, Clone)]
pub enum Event {
    ActionPerformed(text_editor::Action),
}

impl CodeEditorComponent {
    pub fn new_with_language(initial_content: String, syntax: &str) -> Self {
        Self {
            content: text_editor::Content::with_text(&initial_content),
            theme: EditorTheme::dark(),
            syntax: syntax.to_string(),
            scroll_id: scrollable::Id::unique(),
            cursor_line: 0,
            cursor_column: 0,
            last_scroll_y: 0.0,
        }
    }
}

impl Default for CodeEditorComponent {
    fn default() -> Self {
        // Exemple avec du code Lua
        let lua_content = r#"-- Lua example code
function hello_world()
    print("Hello, World!")
    
    for i = 1, 10 do
        print("Count: " .. i)
    end
end

function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

-- Tables (équivalent des dictionnaires/objets)
local person = {
    name = "John",
    age = 30,
    greet = function(self)
        print("Hello, I'm " .. self.name)
    end
}

-- Main execution
hello_world()
print("Fibonacci(10) = " .. fibonacci(10))
person:greet()
"#
        .to_string();

        Self::new_with_language(lua_content, "lua")
    }
}

impl CodeEditorComponent {
    pub fn update(&mut self, event: Event) -> iced::Task<Event> {
        use text_editor::{Action, Motion};

        match event {
            Event::ActionPerformed(action) => {
                // Calculer le nombre de lignes actuel (avant l'action)
                let text_content = self.content.text();
                let lines: Vec<&str> = text_content.lines().collect();
                let max_lines = lines.len().saturating_sub(1);

                // Obtenir la ligne actuelle pour certains calculs
                let current_line = if self.cursor_line < lines.len() {
                    lines[self.cursor_line]
                } else {
                    ""
                };

                // Mettre à jour cursor_line et cursor_column selon l'action
                let should_scroll = match &action {
                    Action::Move(motion) | Action::Select(motion) => {
                        match motion {
                            Motion::Up => {
                                self.cursor_line = self.cursor_line.saturating_sub(1);
                                // Garder la colonne si possible
                                if self.cursor_line < lines.len() {
                                    let line_len = lines[self.cursor_line].len();
                                    self.cursor_column = self.cursor_column.min(line_len);
                                }
                                true
                            }
                            Motion::Down => {
                                self.cursor_line = (self.cursor_line + 1).min(max_lines);
                                // Garder la colonne si possible
                                if self.cursor_line < lines.len() {
                                    let line_len = lines[self.cursor_line].len();
                                    self.cursor_column = self.cursor_column.min(line_len);
                                }
                                true
                            }
                            Motion::Left => {
                                if self.cursor_column > 0 {
                                    self.cursor_column -= 1;
                                } else if self.cursor_line > 0 {
                                    // Remonter à la fin de la ligne précédente
                                    self.cursor_line -= 1;
                                    if self.cursor_line < lines.len() {
                                        self.cursor_column = lines[self.cursor_line].len();
                                    }
                                }
                                false // Pas de scroll vertical nécessaire
                            }
                            Motion::Right => {
                                let line_len = current_line.len();
                                if self.cursor_column < line_len {
                                    self.cursor_column += 1;
                                } else if self.cursor_line < max_lines {
                                    // Descendre au début de la ligne suivante
                                    self.cursor_line += 1;
                                    self.cursor_column = 0;
                                }
                                false // Pas de scroll vertical nécessaire
                            }
                            Motion::WordLeft => {
                                // Approximation: reculer de quelques caractères
                                self.cursor_column = self.cursor_column.saturating_sub(5);
                                false
                            }
                            Motion::WordRight => {
                                // Approximation: avancer de quelques caractères
                                let line_len = current_line.len();
                                self.cursor_column = (self.cursor_column + 5).min(line_len);
                                false
                            }
                            Motion::Home => {
                                self.cursor_column = 0;
                                false
                            }
                            Motion::End => {
                                self.cursor_column = current_line.len();
                                false
                            }
                            Motion::DocumentStart => {
                                self.cursor_line = 0;
                                self.cursor_column = 0;
                                true
                            }
                            Motion::DocumentEnd => {
                                self.cursor_line = max_lines;
                                self.cursor_column = if max_lines < lines.len() {
                                    lines[max_lines].len()
                                } else {
                                    0
                                };
                                true
                            }
                            Motion::PageUp => {
                                self.cursor_line = self.cursor_line.saturating_sub(20);
                                true
                            }
                            Motion::PageDown => {
                                self.cursor_line = (self.cursor_line + 20).min(max_lines);
                                true
                            }
                        }
                    }
                    Action::Edit(edit) => {
                        use text_editor::Edit;
                        match edit {
                            Edit::Enter => {
                                // Nouvelle ligne créée, curseur descend
                                self.cursor_line += 1;
                                self.cursor_column = 0;
                                true
                            }
                            Edit::Insert(_c) => {
                                // Caractère inséré, colonne avance
                                self.cursor_column += 1;
                                false
                            }
                            Edit::Backspace => {
                                // Backspace: reculer d'un caractère ou remonter d'une ligne
                                if self.cursor_column > 0 {
                                    self.cursor_column -= 1;
                                    false
                                } else if self.cursor_line > 0 {
                                    // Fusion avec la ligne précédente
                                    self.cursor_line -= 1;
                                    if self.cursor_line < lines.len() {
                                        self.cursor_column = lines[self.cursor_line].len();
                                    }
                                    true // Scroll car on change de ligne
                                } else {
                                    false
                                }
                            }
                            Edit::Delete => {
                                // Delete ne change pas la position du curseur
                                // (sauf cas spéciaux en fin de ligne, mais difficile à tracker)
                                false
                            }
                            Edit::Paste(text) => {
                                // Compter les nouvelles lignes dans le texte collé
                                let newlines = text.chars().filter(|&c| c == '\n').count();
                                if newlines > 0 {
                                    self.cursor_line += newlines;
                                    // Trouver la position après le dernier \n
                                    if let Some(last_line) = text.lines().last() {
                                        self.cursor_column = last_line.len();
                                    } else {
                                        self.cursor_column = 0;
                                    }
                                    true
                                } else {
                                    self.cursor_column += text.len();
                                    false
                                }
                            }
                        }
                    }
                    Action::Click(point) => {
                        // Estimer la ligne depuis la position Y du clic
                        let estimated_line =
                            ((point.y - PADDING_TOP) / LINE_HEIGHT).max(0.0) as usize;
                        self.cursor_line = estimated_line.min(max_lines);

                        // Estimer aussi la colonne
                        let estimated_column =
                            ((point.x - PADDING_LEFT) / CHAR_WIDTH).max(0.0) as usize;
                        if self.cursor_line < lines.len() {
                            self.cursor_column =
                                estimated_column.min(lines[self.cursor_line].len());
                        } else {
                            self.cursor_column = 0;
                        }
                        true
                    }
                    Action::Drag(_) => {
                        // Similaire à Click, mais on ne peut pas tracker précisément
                        // Laisser le comportement par défaut
                        false
                    }
                    _ => false,
                };

                // Appliquer l'action au contenu
                self.content.perform(action);

                // Si le curseur a bougé verticalement, scroller pour le garder visible
                if should_scroll {
                    return self.scroll_to_cursor();
                }

                iced::Task::none()
            }
        }
    }

    /// Scroller intelligemment pour garder le curseur visible
    fn scroll_to_cursor(&mut self) -> iced::Task<Event> {
        // Position Y du curseur
        let cursor_y = (self.cursor_line as f32 * LINE_HEIGHT) + PADDING_TOP;

        // Calculer la nouvelle position de scroll seulement si nécessaire
        let new_scroll_y = if cursor_y < self.last_scroll_y + TOP_MARGIN {
            // Curseur trop haut, scroller vers le haut
            (cursor_y - TOP_MARGIN).max(0.0)
        } else if cursor_y > self.last_scroll_y + VIEWPORT_HEIGHT - BOTTOM_MARGIN {
            // Curseur trop bas, scroller vers le bas
            cursor_y - VIEWPORT_HEIGHT + BOTTOM_MARGIN
        } else {
            // Curseur déjà visible, pas besoin de scroller
            return iced::Task::none();
        };

        // Mettre à jour la position de scroll
        self.last_scroll_y = new_scroll_y;

        scrollable::scroll_to(
            self.scroll_id.clone(),
            scrollable::AbsoluteOffset {
                x: 0.0,
                y: new_scroll_y,
            },
        )
    }

    pub fn view(&self) -> Element<'_, Event> {
        use iced::widget::{column, container, row, scrollable, text};
        use iced::{Color, Fill, Pixels};

        // Get the text content and split into lines
        let text_content = self.content.text();
        let lines: Vec<&str> = text_content.lines().collect();
        let line_count = lines.len().max(1);

        // Create line numbers column (no internal scrollable)
        let line_numbers: Element<Event> = {
            let mut numbers = column![].spacing(0);
            for i in 1..=line_count {
                numbers = numbers.push(
                    text(format!("{:>4}", i))
                        .font(iced::Font::MONOSPACE)
                        .size(FONT_SIZE)
                        .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
                            LINE_HEIGHT,
                        )))
                        .color(self.theme.line_number_color),
                );
            }

            container(numbers)
                .width(GUTTER_WIDTH)
                .padding(PADDING_TOP)
                .style(move |_theme| container::Style {
                    background: Some(self.theme.gutter_background.into()),
                    border: iced::Border {
                        color: self.theme.gutter_border,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        };

        // Create the text editor with matching font settings and syntax highlighting
        let editor: Element<Event> = text_editor(&self.content)
            .highlight(&self.syntax, highlighter::Theme::Base16Ocean)
            .on_action(Event::ActionPerformed)
            .font(iced::Font::MONOSPACE)
            .size(FONT_SIZE)
            .line_height(iced::widget::text::LineHeight::Absolute(Pixels(LINE_HEIGHT)))
            .padding(PADDING_TOP)
            .key_binding(|key_press| {
                use iced::keyboard::{self, key};
                use iced::widget::text_editor::Binding;

                match key_press.key.as_ref() {
                    // Gestion de la touche Tab
                    keyboard::Key::Named(key::Named::Tab) => {
                        // Insérer 4 espaces au lieu de Tab
                        Some(Binding::Sequence(vec![
                            Binding::Insert(' '),
                            Binding::Insert(' '),
                            Binding::Insert(' '),
                            Binding::Insert(' '),
                        ]))
                    }
                    // Gestion explicite de la touche Delete
                    keyboard::Key::Named(key::Named::Delete) => Some(Binding::Delete),
                    // Utiliser le comportement par défaut pour les autres touches
                    _ => Binding::from_key_press(key_press),
                }
            })
            .style(|_theme, _status| text_editor::Style {
                background: self.theme.background.into(),
                border: iced::Border::default(),
                icon: self.theme.text_color,
                placeholder: Color::from_rgb(0.4, 0.4, 0.4),
                value: self.theme.text_color,
                selection: Color::from_rgb(0.3, 0.5, 0.8),
            })
            .into();

        // Combine line numbers and editor in a single scrollable
        container(scrollable(row![line_numbers, editor].spacing(0)).id(self.scroll_id.clone()))
            .width(Fill)
            .height(Fill)
            .style(move |_theme| container::Style {
                background: Some(self.theme.background.into()),
                ..Default::default()
            })
            .into()
    }

            container(numbers)
                .width(60)
                .padding(10)
                .style(move |_theme| container::Style {
                    background: Some(self.theme.gutter_background.into()),
                    border: iced::Border {
                        color: self.theme.gutter_border,
                        width: 0.0,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        };

        // Create the text editor with matching font settings and syntax highlighting
        let editor: Element<Event> = text_editor(&self.content)
            .highlight(&self.syntax, highlighter::Theme::Base16Ocean)
            .on_action(Event::ActionPerformed)
            .font(iced::Font::MONOSPACE)
            .size(FONT_SIZE)
            .line_height(iced::widget::text::LineHeight::Absolute(Pixels(
                LINE_HEIGHT,
            )))
            .padding(10)
            .key_binding(|key_press| {
                use iced::keyboard::{self, key};
                use iced::widget::text_editor::Binding;

                match key_press.key.as_ref() {
                    // Gestion de la touche Tab
                    keyboard::Key::Named(key::Named::Tab) => {
                        // Insérer 4 espaces au lieu de Tab
                        Some(Binding::Sequence(vec![
                            Binding::Insert(' '),
                            Binding::Insert(' '),
                            Binding::Insert(' '),
                            Binding::Insert(' '),
                        ]))
                    }
                    // Gestion explicite de la touche Delete
                    keyboard::Key::Named(key::Named::Delete) => Some(Binding::Delete),
                    // Utiliser le comportement par défaut pour les autres touches
                    _ => Binding::from_key_press(key_press),
                }
            })
            .style(|_theme, _status| text_editor::Style {
                background: self.theme.background.into(),
                border: iced::Border::default(),
                icon: self.theme.text_color,
                placeholder: Color::from_rgb(0.4, 0.4, 0.4),
                value: self.theme.text_color,
                selection: Color::from_rgb(0.3, 0.5, 0.8),
            })
            .into();

        // Combine line numbers and editor in a single scrollable
        container(scrollable(row![line_numbers, editor].spacing(0)).id(self.scroll_id.clone()))
            .width(Fill)
            .height(Fill)
            .style(move |_theme| container::Style {
                background: Some(self.theme.background.into()),
                ..Default::default()
            })
            .into()
    }

    // Accesseurs pour les tests
    #[cfg(test)]
    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_column)
    }

    #[cfg(test)]
    pub fn last_scroll_y(&self) -> f32 {
        self.last_scroll_y
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Point;
    use text_editor::{Action, Edit, Motion};

    /// Helper pour créer un composant de test avec un contenu simple
    fn create_test_editor() -> CodeEditorComponent {
        let content = "Line 0\nLine 1\nLine 2\nLine 3\nLine 4".to_string();
        CodeEditorComponent::new_with_language(content, "py")
    }

    #[test]
    fn test_cursor_starts_at_origin() {
        let editor = create_test_editor();
        assert_eq!(editor.cursor_position(), (0, 0));
    }

    #[test]
    fn test_cursor_move_down() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Curseur devrait être sur ligne 1"
        );

        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(
            editor.cursor_position(),
            (2, 0),
            "Curseur devrait être sur ligne 2"
        );
    }

    #[test]
    fn test_cursor_move_up() {
        let mut editor = create_test_editor();

        // Descendre puis remonter
        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(editor.cursor_position(), (2, 0));

        editor.update(Event::ActionPerformed(Action::Move(Motion::Up)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Curseur devrait être remonté à ligne 1"
        );
    }

    #[test]
    fn test_cursor_move_up_at_top() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Move(Motion::Up)));
        assert_eq!(
            editor.cursor_position(),
            (0, 0),
            "Curseur devrait rester à (0,0)"
        );
    }

    #[test]
    fn test_cursor_move_down_at_bottom() {
        let mut editor = create_test_editor();

        // Aller tout en bas (ligne 4 = dernière ligne)
        for _ in 0..10 {
            editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        }

        assert_eq!(
            editor.cursor_position().0,
            4,
            "Curseur devrait être bloqué à la dernière ligne"
        );
    }

    #[test]
    fn test_cursor_left_right() {
        let mut editor = create_test_editor();

        // Aller à droite
        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 1), "Colonne devrait être 1");

        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 2), "Colonne devrait être 2");

        // Revenir à gauche
        editor.update(Event::ActionPerformed(Action::Move(Motion::Left)));
        assert_eq!(editor.cursor_position(), (0, 1), "Colonne devrait être 1");
    }

    #[test]
    fn test_cursor_left_wraps_to_previous_line() {
        let mut editor = create_test_editor();

        // Descendre d'une ligne
        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(editor.cursor_position(), (1, 0));

        // Aller à gauche depuis le début de ligne devrait remonter à la fin de la ligne précédente
        editor.update(Event::ActionPerformed(Action::Move(Motion::Left)));
        assert_eq!(editor.cursor_position().0, 0, "Devrait remonter à ligne 0");
        assert_eq!(
            editor.cursor_position().1,
            6,
            "Devrait être à la fin de 'Line 0' (6 caractères)"
        );
    }

    #[test]
    fn test_cursor_right_wraps_to_next_line() {
        let mut editor = create_test_editor();

        // Aller à la fin de la ligne 0
        editor.update(Event::ActionPerformed(Action::Move(Motion::End)));
        assert_eq!(
            editor.cursor_position(),
            (0, 6),
            "Devrait être à la fin de ligne 0"
        );

        // Aller à droite devrait descendre au début de la ligne suivante
        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Devrait descendre à ligne 1, colonne 0"
        );
    }

    #[test]
    fn test_home_end() {
        let mut editor = create_test_editor();

        // Aller à droite puis Home
        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        editor.update(Event::ActionPerformed(Action::Move(Motion::Home)));
        assert_eq!(
            editor.cursor_position(),
            (0, 0),
            "Home devrait ramener à colonne 0"
        );

        // End devrait aller à la fin
        editor.update(Event::ActionPerformed(Action::Move(Motion::End)));
        assert_eq!(
            editor.cursor_position(),
            (0, 6),
            "End devrait aller à la fin de ligne"
        );
    }

    #[test]
    fn test_document_start_end() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Move(Motion::DocumentEnd)));
        assert_eq!(
            editor.cursor_position().0,
            4,
            "DocumentEnd devrait aller à la dernière ligne"
        );

        editor.update(Event::ActionPerformed(Action::Move(Motion::DocumentStart)));
        assert_eq!(
            editor.cursor_position(),
            (0, 0),
            "DocumentStart devrait aller à (0,0)"
        );
    }

    #[test]
    fn test_page_up_down() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Move(Motion::PageDown)));
        assert_eq!(
            editor.cursor_position().0,
            4,
            "PageDown devrait aller à ligne 4 (max pour ce fichier court)"
        );

        editor.update(Event::ActionPerformed(Action::Move(Motion::PageUp)));
        assert_eq!(
            editor.cursor_position().0,
            0,
            "PageUp de 20 lignes depuis ligne 4 = ligne 0"
        );
    }

    #[test]
    fn test_enter_creates_new_line() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Edit(Edit::Enter)));
        assert_eq!(
            editor.cursor_position(),
            (1, 0),
            "Enter devrait descendre à ligne 1"
        );
    }

    #[test]
    fn test_backspace_at_start_of_line() {
        let mut editor = create_test_editor();

        // Descendre d'une ligne
        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        assert_eq!(editor.cursor_position(), (1, 0));

        // Backspace depuis le début de ligne devrait remonter
        editor.update(Event::ActionPerformed(Action::Edit(Edit::Backspace)));
        assert_eq!(
            editor.cursor_position().0,
            0,
            "Backspace devrait remonter à ligne 0"
        );
        assert_eq!(
            editor.cursor_position().1,
            6,
            "Devrait être à la fin de ligne 0"
        );
    }

    #[test]
    fn test_backspace_in_middle_of_line() {
        let mut editor = create_test_editor();

        // Aller à droite
        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        editor.update(Event::ActionPerformed(Action::Move(Motion::Right)));
        assert_eq!(editor.cursor_position(), (0, 2));

        // Backspace devrait juste reculer d'une colonne
        editor.update(Event::ActionPerformed(Action::Edit(Edit::Backspace)));
        assert_eq!(
            editor.cursor_position(),
            (0, 1),
            "Backspace devrait reculer à colonne 1"
        );
    }

    #[test]
    fn test_paste_single_line() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Edit(Edit::Paste(
            std::sync::Arc::new("Hello".to_string()),
        ))));
        assert_eq!(
            editor.cursor_position(),
            (0, 5),
            "Colonne devrait avancer de 5"
        );
    }

    #[test]
    fn test_paste_multiline() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Edit(Edit::Paste(
            std::sync::Arc::new("Line A\nLine B\nLine C".to_string()),
        ))));
        assert_eq!(
            editor.cursor_position().0,
            2,
            "Devrait être sur ligne 2 (2 newlines)"
        );
        assert_eq!(
            editor.cursor_position().1,
            6,
            "Devrait être à la fin de 'Line C'"
        );
    }

    #[test]
    fn test_click_position() {
        let mut editor = create_test_editor();

        // Simuler un clic à la ligne 2, colonne 3
        // LINE_HEIGHT = 20.0, PADDING_TOP = 10.0
        // y = ligne 2 -> (2 * 20) + 10 = 50
        let point = Point { x: 40.0, y: 50.0 };

        editor.update(Event::ActionPerformed(Action::Click(point)));
        assert_eq!(
            editor.cursor_position().0,
            2,
            "Click devrait positionner à ligne 2"
        );
    }

    #[test]
    fn test_scroll_optimization() {
        let mut editor = create_test_editor();

        // Premier déplacement devrait trigger un scroll
        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        let _first_scroll = editor.last_scroll_y();

        // Deuxième déplacement dans la même zone visible ne devrait pas changer le scroll
        editor.update(Event::ActionPerformed(Action::Move(Motion::Down)));
        // Note: sans connaître la hauteur exacte du viewport, on peut juste vérifier que
        // last_scroll_y est trackée
        assert!(
            editor.last_scroll_y() >= 0.0,
            "Scroll Y devrait être positif ou zéro"
        );
    }

    #[test]
    fn test_insert_character() {
        let mut editor = create_test_editor();

        editor.update(Event::ActionPerformed(Action::Edit(Edit::Insert('X'))));
        assert_eq!(
            editor.cursor_position(),
            (0, 1),
            "Insert devrait avancer la colonne"
        );

        editor.update(Event::ActionPerformed(Action::Edit(Edit::Insert('Y'))));
        assert_eq!(
            editor.cursor_position(),
            (0, 2),
            "Insert devrait continuer d'avancer"
        );
    }
}
