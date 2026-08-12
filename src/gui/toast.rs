use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::font::Weight;
use iced::mouse;
use iced::widget::{button, column, container, row, space, text};
use iced::{
    Alignment, Border, Color, Element, Event, Fill, Length, Padding, Point, Rectangle, Renderer,
    Size, Theme, Vector,
};

use super::message::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastId(u64);

impl ToastId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ToastStatus {
    Success,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Toast {
    pub(crate) id: ToastId,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) status: ToastStatus,
}

impl Toast {
    pub(crate) fn success(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastStatus::Success)
    }

    pub(crate) fn danger(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastStatus::Danger)
    }

    fn new(title: impl Into<String>, body: impl Into<String>, status: ToastStatus) -> Self {
        Self {
            id: ToastId::new(0),
            title: title.into(),
            body: body.into(),
            status,
        }
    }

    pub(crate) fn with_id(mut self, id: ToastId) -> Self {
        self.id = id;
        self
    }
}

pub(crate) struct Manager<'a> {
    content: Element<'a, Message>,
    toasts: Vec<Element<'a, Message>>,
}

impl<'a> Manager<'a> {
    pub(crate) fn new(content: impl Into<Element<'a, Message>>, toasts: &'a [Toast]) -> Self {
        Self {
            content: content.into(),
            toasts: toasts.iter().rev().take(3).map(toast_view).collect(),
        }
    }
}

fn toast_view(toast: &Toast) -> Element<'_, Message> {
    let status_color = match toast.status {
        ToastStatus::Success => Color::from_rgb(0.35, 0.78, 0.57),
        ToastStatus::Danger => Color::from_rgb(0.94, 0.39, 0.42),
    };
    let style = match toast.status {
        ToastStatus::Success => success_style,
        ToastStatus::Danger => danger_style,
    };

    container(
        column![
            row![
                text(&toast.title)
                    .size(15)
                    .font(iced::Font {
                        weight: Weight::Semibold,
                        ..iced::Font::default()
                    })
                    .color(status_color),
                space::horizontal(),
                button(text("Dismiss").size(12))
                    .padding(Padding::from([4, 6]))
                    .style(button::text)
                    .on_press(Message::DismissToast(toast.id)),
            ]
            .spacing(12)
            .align_y(iced::Alignment::Center),
            text(&toast.body)
                .size(13)
                .color(Color::from_rgb(0.92, 0.93, 0.96)),
        ]
        .spacing(7),
    )
    .width(360)
    .padding(14)
    .style(style)
    .into()
}

fn success_style(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.075, 0.12, 0.10))
        .border(Border {
            color: Color::from_rgb(0.18, 0.42, 0.30),
            width: 1.0,
            radius: 12.0.into(),
        })
}

fn danger_style(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.16, 0.075, 0.085))
        .border(Border {
            color: Color::from_rgb(0.38, 0.14, 0.16),
            width: 1.0,
            radius: 12.0.into(),
        })
}

impl Widget<Message, Theme, Renderer> for Manager<'_> {
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn children(&self) -> Vec<Tree> {
        std::iter::once(Tree::new(&self.content))
            .chain(self.toasts.iter().map(Tree::new))
            .collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(
            &std::iter::once(&self.content)
                .chain(self.toasts.iter())
                .collect::<Vec<_>>(),
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
                renderer,
                operation,
            );
        });
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
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
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let (content_state, toasts_state) = tree.children.split_at_mut(1);
        let content = self.content.as_widget_mut().overlay(
            &mut content_state[0],
            layout,
            renderer,
            viewport,
            translation,
        );
        let toasts = (!self.toasts.is_empty()).then(|| {
            overlay::Element::new(Box::new(ToastOverlay {
                position: layout.bounds().position() + translation,
                viewport: *viewport,
                toasts: &mut self.toasts,
                trees: toasts_state,
            }))
        });
        let overlays = content.into_iter().chain(toasts).collect::<Vec<_>>();

        (!overlays.is_empty()).then(|| overlay::Group::with_children(overlays).overlay())
    }
}

struct ToastOverlay<'a, 'b> {
    position: Point,
    viewport: Rectangle,
    toasts: &'b mut [Element<'a, Message>],
    trees: &'b mut [Tree],
}

impl overlay::Overlay<Message, Theme, Renderer> for ToastOverlay<'_, '_> {
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::ZERO, bounds);

        layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            &limits,
            Fill,
            Fill,
            26.into(),
            10.0,
            Alignment::End,
            self.toasts,
            self.trees,
        )
        .translate(Vector::new(self.position.x, self.position.y))
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let viewport = layout.bounds();

        for ((child, state), layout) in self
            .toasts
            .iter_mut()
            .zip(self.trees.iter_mut())
            .zip(layout.children())
        {
            let mut local_messages = Vec::new();
            let mut local_shell = Shell::new(&mut local_messages);

            child.as_widget_mut().update(
                state,
                event,
                layout,
                cursor,
                renderer,
                clipboard,
                &mut local_shell,
                &viewport,
            );
            shell.merge(local_shell, std::convert::identity);
        }
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let viewport = layout.bounds();

        for ((child, tree), layout) in self
            .toasts
            .iter()
            .zip(self.trees.iter())
            .zip(layout.children())
        {
            child
                .as_widget()
                .draw(tree, renderer, theme, style, layout, cursor, &viewport);
        }
    }

    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        operation.container(None, layout.bounds());
        operation.traverse(&mut |operation| {
            self.toasts
                .iter_mut()
                .zip(self.trees.iter_mut())
                .zip(layout.children())
                .for_each(|((child, state), layout)| {
                    child
                        .as_widget_mut()
                        .operate(state, layout, renderer, operation);
                });
        });
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.toasts
            .iter()
            .zip(self.trees.iter())
            .zip(layout.children())
            .map(|((child, state), layout)| {
                child
                    .as_widget()
                    .mouse_interaction(state, layout, cursor, &self.viewport, renderer)
                    .max(if cursor.is_over(layout.bounds()) {
                        mouse::Interaction::Idle
                    } else {
                        mouse::Interaction::None
                    })
            })
            .max()
            .unwrap_or_default()
    }
}

impl<'a> From<Manager<'a>> for Element<'a, Message> {
    fn from(manager: Manager<'a>) -> Self {
        Element::new(manager)
    }
}
