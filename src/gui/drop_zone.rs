use iced::widget::canvas::{self, LineCap, LineDash, LineJoin, Path, Stroke};
use iced::{Element, Fill, Point, Rectangle, Renderer, Size, Theme, border, mouse};

use super::style::{ACCENT, DROP_BORDER, INSET_BACKGROUND};

const DASH_PATTERN: &[f32] = &[7.0, 6.0];

pub(crate) fn chrome<'a, Message: 'a>(active: bool) -> Element<'a, Message> {
    canvas::Canvas::new(DropZoneChrome { active })
        .width(Fill)
        .height(Fill)
        .into()
}

#[derive(Debug, Clone, Copy)]
struct DropZoneChrome {
    active: bool,
}

impl<Message> canvas::Program<Message> for DropZoneChrome {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let outline = Path::rounded_rectangle(
            Point::new(1.0, 1.0),
            Size::new(
                (bounds.width - 2.0).max(0.0),
                (bounds.height - 2.0).max(0.0),
            ),
            border::Radius::from(12.0),
        );
        frame.fill(&outline, INSET_BACKGROUND);
        frame.stroke(
            &outline,
            Stroke {
                width: if self.active { 1.6 } else { 1.2 },
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                line_dash: LineDash {
                    segments: DASH_PATTERN,
                    offset: 0,
                },
                ..Stroke::default()
            }
            .with_color(if self.active { ACCENT } else { DROP_BORDER }),
        );
        vec![frame.into_geometry()]
    }
}
