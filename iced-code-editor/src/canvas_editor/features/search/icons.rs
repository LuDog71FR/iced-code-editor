//! Small, consistently stroked search toolbar symbols.
use crate::canvas_editor::Message;
use iced::widget::canvas::{self, Canvas, Path, Stroke};
use iced::{Element, Point, Rectangle, Renderer, Theme, mouse};

#[derive(Clone, Copy)]
pub(super) enum Symbol {
    Case,
    Word,
    Regex,
    ReplaceToggle,
    Multiline,
    Replace,
    ReplaceAll,
    Previous,
    Next,
    Close,
    SelectAll,
}

pub(super) fn view(
    symbol: Symbol,
    active: bool,
    enabled: bool,
) -> Element<'static, Message> {
    let size = if matches!(symbol, Symbol::Case | Symbol::Word | Symbol::Regex)
    {
        14
    } else {
        16
    };
    Canvas::new(Icon { symbol, active, enabled })
        .width(size)
        .height(size)
        .into()
}
struct Icon {
    symbol: Symbol,
    active: bool,
    enabled: bool,
}
impl canvas::Program<Message> for Icon {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.scale(bounds.width / 20.0);
        let palette = theme.extended_palette();
        let mut color = if self.active
            && matches!(self.symbol, Symbol::ReplaceToggle | Symbol::Multiline)
        {
            palette.primary.strong.color
        } else {
            palette.background.base.text
        };
        if !self.enabled {
            color.a *= 0.35;
        }
        let stroke = Stroke::default()
            .with_color(color)
            .with_width(1.5)
            .with_line_cap(canvas::LineCap::Round)
            .with_line_join(canvas::LineJoin::Round);
        let mut line = |points: &[(f32, f32)]| {
            let path = Path::new(|p| {
                if let Some(&(x, y)) = points.first() {
                    p.move_to(Point::new(x, y));
                }
                for &(x, y) in points.iter().skip(1) {
                    p.line_to(Point::new(x, y));
                }
            });
            frame.stroke(&path, stroke);
        };
        match self.symbol {
            Symbol::Previous => line(&[(12., 5.), (7., 10.), (12., 15.)]),
            Symbol::Next => line(&[(8., 5.), (13., 10.), (8., 15.)]),
            Symbol::Close => {
                line(&[(5., 5.), (15., 15.)]);
                line(&[(15., 5.), (5., 15.)]);
            }
            Symbol::Multiline => {
                line(&[(3., 5.), (17., 5.)]);
                line(&[(3., 10.), (12., 10.)]);
                line(&[(3., 15.), (10., 15.)]);
                if self.active {
                    line(&[(17., 16.), (14., 13.), (17., 10.)]);
                } else {
                    line(&[(14., 12.), (17., 15.), (14., 18.)]);
                }
            }
            Symbol::SelectAll => {
                // S + asterisk, matching the search toolbar's R / R* vocabulary.
                line(&[
                    (9., 4.),
                    (7.5, 3.),
                    (4.5, 3.),
                    (3., 4.5),
                    (3.5, 6.5),
                    (7.5, 8.),
                    (8.5, 10.),
                    (7., 12.),
                    (4., 12.),
                    (2.5, 11.),
                ]);
                line(&[(14.5, 10.), (14.5, 18.)]);
                line(&[(11., 12.), (18., 16.)]);
                line(&[(11., 16.), (18., 12.)]);
            }
            Symbol::Regex => {
                line(&[(13., 3.), (13., 11.)]);
                line(&[(9.5, 5.), (16.5, 9.)]);
                line(&[(9.5, 9.), (16.5, 5.)]);
                frame.fill(&Path::circle(Point::new(5., 15.), 1.4), color);
            }
            Symbol::Case | Symbol::Word => {
                if matches!(self.symbol, Symbol::Word) {
                    line(&[(3., 16.5), (17., 16.5)]);
                }
                frame.fill_text(canvas::Text {
                    content: if matches!(self.symbol, Symbol::Case) {
                        "Aa"
                    } else {
                        "wd"
                    }
                    .into(),
                    position: Point::new(
                        10.,
                        if matches!(self.symbol, Symbol::Word) {
                            8.5
                        } else {
                            10.
                        },
                    ),
                    color,
                    size: 13.into(),
                    align_x: iced::alignment::Horizontal::Center.into(),
                    align_y: iced::alignment::Vertical::Center,
                    font: iced::Font {
                        weight: iced::font::Weight::Semibold,
                        ..iced::Font::DEFAULT
                    },
                    ..Default::default()
                });
            }
            Symbol::ReplaceToggle | Symbol::Replace | Symbol::ReplaceAll => {
                // R and an arrow/star describe replacement rather than refresh or swap.
                line(&[
                    (3., 12.),
                    (3., 3.),
                    (7., 3.),
                    (9., 4.5),
                    (9., 7.),
                    (7., 8.),
                    (3., 8.),
                ]);
                line(&[(6., 8.), (9., 12.)]);
                match self.symbol {
                    Symbol::ReplaceToggle => {
                        frame.stroke(
                            &Path::new(|p| {
                                p.move_to(Point::new(12., 4.));
                                p.bezier_curve_to(
                                    Point::new(19., 5.),
                                    Point::new(19., 16.),
                                    Point::new(11., 16.),
                                );
                            }),
                            stroke,
                        );
                        frame.stroke(
                            &Path::new(|p| {
                                p.move_to(Point::new(13., 13.));
                                p.line_to(Point::new(10., 16.));
                                p.line_to(Point::new(13., 19.));
                            }),
                            stroke,
                        );
                    }
                    Symbol::Replace => {
                        line(&[(10., 15.), (18., 15.)]);
                        line(&[(15., 12.), (18., 15.), (15., 18.)]);
                    }
                    Symbol::ReplaceAll => {
                        line(&[(14.5, 11.), (14.5, 18.)]);
                        line(&[(11.5, 12.8), (17.5, 16.3)]);
                        line(&[(11.5, 16.3), (17.5, 12.8)]);
                    }
                    _ => {}
                }
            }
        }
        vec![frame.into_geometry()]
    }
}
