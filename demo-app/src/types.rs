use iced::Font;
use iced_code_editor::Language;

/// Identifier for which editor is being referenced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorId {
    Left,
    Right,
}

/// Wrapper for Font to implement Display trait for pick_list.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FontOption {
    pub name: &'static str,
    pub font: Font,
}

impl FontOption {
    pub const MONOSPACE: FontOption =
        FontOption { name: "Monospace (Default)", font: Font::MONOSPACE };

    pub const SERIF: FontOption = FontOption {
        name: "Serif",
        font: Font { family: iced::font::Family::Serif, ..Font::DEFAULT },
    };

    pub const SANS_SERIF: FontOption = FontOption {
        name: "Sans Serif",
        font: Font { family: iced::font::Family::SansSerif, ..Font::DEFAULT },
    };

    pub const JETBRAINS_MONO: FontOption = FontOption {
        name: "JetBrains Mono",
        font: Font {
            family: iced::font::Family::Name("JetBrains Mono"),
            ..Font::DEFAULT
        },
    };

    pub const ALL: [FontOption; 4] = [
        FontOption::MONOSPACE,
        FontOption::SERIF,
        FontOption::SANS_SERIF,
        FontOption::JETBRAINS_MONO,
    ];

    pub fn font(&self) -> Font {
        self.font
    }
}

impl std::fmt::Display for FontOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Wrapper for Language to implement Display trait for pick_list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageOption(Language);

impl LanguageOption {
    pub const ALL: [LanguageOption; 8] = [
        LanguageOption(Language::German),
        LanguageOption(Language::English),
        LanguageOption(Language::Spanish),
        LanguageOption(Language::French),
        LanguageOption(Language::Italian),
        LanguageOption(Language::PortugueseBR),
        LanguageOption(Language::PortuguesePT),
        LanguageOption(Language::ChineseSimplified),
    ];

    pub fn inner(&self) -> Language {
        self.0
    }
}

impl From<Language> for LanguageOption {
    fn from(lang: Language) -> Self {
        LanguageOption(lang)
    }
}

impl std::fmt::Display for LanguageOption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Language::English => write!(f, "English"),
            Language::French => write!(f, "Français"),
            Language::Spanish => write!(f, "Español"),
            Language::German => write!(f, "Deutsch"),
            Language::Italian => write!(f, "Italiano"),
            Language::PortugueseBR => write!(f, "Português (BR)"),
            Language::PortuguesePT => write!(f, "Português (PT)"),
            Language::ChineseSimplified => write!(f, "简体中文"),
        }
    }
}

/// Code templates available in the dropdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    Empty,
    HelloWorld,
    Fibonacci,
    Factorial,
}

impl Template {
    pub const ALL: [Template; 4] = [
        Template::Empty,
        Template::HelloWorld,
        Template::Fibonacci,
        Template::Factorial,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Template::Empty => "Empty",
            Template::HelloWorld => "Hello World",
            Template::Fibonacci => "Fibonacci",
            Template::Factorial => "Factorial",
        }
    }

    pub fn content(&self) -> &'static str {
        match self {
            Template::Empty => "",
            Template::HelloWorld => {
                r#"-- Hello World in Lua
print("Hello, World!")
"#
            }
            Template::Fibonacci => {
                r#"-- Fibonacci sequence in Lua
function fibonacci(n)
    if n <= 1 then
        return n
    end
    return fibonacci(n - 1) + fibonacci(n - 2)
end

-- Print first 10 Fibonacci numbers
for i = 0, 10 do
    print("fib(" .. i .. ") = " .. fibonacci(i))
end
"#
            }
            Template::Factorial => {
                r#"-- Factorial function in Lua
function factorial(n)
    if n <= 1 then
        return 1
    end
    return n * factorial(n - 1)
end

-- Calculate factorials
for i = 1, 10 do
    print(i .. "! = " .. factorial(i))
end
"#
            }
        }
    }
}

impl std::fmt::Display for Template {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Pane content types for the editor PaneGrid.
#[derive(Debug, Clone, Copy)]
pub enum PaneType {
    EditorLeft,
    EditorRight,
}
