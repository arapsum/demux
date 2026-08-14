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
use std::time::Duration;

use super::message::Message as AppMessage;

const SUCCESS_DURATION: Duration = Duration::from_secs(6);
const WARNING_DURATION: Duration = Duration::from_secs(8);
const FAILURE_DURATION: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
pub enum Message {
    Dismiss(ToastId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastId(u64);

impl ToastId {
    pub(crate) const fn new(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastStatus {
    Success,
    Warning,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub(crate) id: ToastId,
    pub(crate) title: String,
    pub(crate) body: String,
    pub(crate) status: ToastStatus,
}

#[derive(Debug)]
pub struct Notifications {
    toasts: Vec<Toast>,
    next_id: u64,
}

impl Notifications {
    pub(crate) const fn new() -> Self {
        Self {
            toasts: Vec::new(),
            next_id: 1,
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Dismiss(id) => {
                self.toasts.retain(|toast| toast.id != id);
                iced::Task::none()
            }
        }
    }

    pub(crate) fn success(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> iced::Task<Message> {
        self.push(Toast::success(title, body), SUCCESS_DURATION)
    }

    pub(crate) fn failure(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> iced::Task<Message> {
        self.push(Toast::danger(title, body), FAILURE_DURATION)
    }

    pub(crate) fn warning(
        &mut self,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> iced::Task<Message> {
        self.push(Toast::warning(title, body), WARNING_DURATION)
    }

    fn push(&mut self, toast: Toast, duration: Duration) -> iced::Task<Message> {
        let id = ToastId::new(self.next_id);
        self.next_id += 1;
        self.toasts.push(toast.with_id(id));

        iced::Task::perform(dismiss_after(id, duration), Message::Dismiss)
    }

    pub(crate) fn view<'a>(
        &'a self,
        content: impl Into<Element<'a, AppMessage>>,
    ) -> Element<'a, AppMessage> {
        Manager::new(content, &self.toasts).into()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.toasts.len()
    }
}

async fn dismiss_after(id: ToastId, duration: Duration) -> ToastId {
    tokio::time::sleep(duration).await;
    id
}

impl Toast {
    pub(crate) fn success(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastStatus::Success)
    }

    pub(crate) fn danger(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastStatus::Danger)
    }

    pub(crate) fn warning(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self::new(title, body, ToastStatus::Warning)
    }

    fn new(title: impl Into<String>, body: impl Into<String>, status: ToastStatus) -> Self {
        Self {
            id: ToastId::new(0),
            title: title.into(),
            body: body.into(),
            status,
        }
    }

    pub(crate) const fn with_id(mut self, id: ToastId) -> Self {
        self.id = id;
        self
    }
}

pub struct Manager<'a> {
    content: Element<'a, AppMessage>,
    toasts: Vec<Element<'a, AppMessage>>,
}

impl<'a> Manager<'a> {
    pub(crate) fn new(content: impl Into<Element<'a, AppMessage>>, toasts: &'a [Toast]) -> Self {
        Self {
            content: content.into(),
            toasts: toasts.iter().rev().take(3).map(toast_view).collect(),
        }
    }
}

fn toast_view(toast: &Toast) -> Element<'_, AppMessage> {
    let status_color = match toast.status {
        ToastStatus::Success => Color::from_rgb(0.35, 0.78, 0.57),
        ToastStatus::Warning => Color::from_rgb(0.96, 0.68, 0.30),
        ToastStatus::Danger => Color::from_rgb(0.94, 0.39, 0.42),
    };
    let style = match toast.status {
        ToastStatus::Success => success_style,
        ToastStatus::Warning => warning_style,
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
                    .on_press(AppMessage::Notifications(Message::Dismiss(toast.id))),
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

fn warning_style(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.14, 0.105, 0.055))
        .border(Border {
            color: Color::from_rgb(0.42, 0.29, 0.10),
            width: 1.0,
            radius: 12.0.into(),
        })
}

impl Widget<AppMessage, Theme, Renderer> for Manager<'_> {
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
        shell: &mut Shell<'_, AppMessage>,
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
    ) -> Option<overlay::Element<'b, AppMessage, Theme, Renderer>> {
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
    toasts: &'b mut [Element<'a, AppMessage>],
    trees: &'b mut [Tree],
}

impl overlay::Overlay<AppMessage, Theme, Renderer> for ToastOverlay<'_, '_> {
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
        shell: &mut Shell<'_, AppMessage>,
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

impl<'a> From<Manager<'a>> for Element<'a, AppMessage> {
    fn from(manager: Manager<'a>) -> Self {
        Element::new(manager)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_notification_keeps_its_content_and_status() {
        let mut notifications = Notifications::new();

        let _ = notifications.success(
            "Ripping complete",
            "example.mp3 is ready in your output folder.",
        );

        assert_eq!(notifications.toasts.len(), 1);
        assert_eq!(notifications.toasts[0].title, "Ripping complete");
        assert!(notifications.toasts[0].body.contains("example.mp3"));
        assert_eq!(notifications.toasts[0].status, ToastStatus::Success);
    }

    #[test]
    fn dismisses_only_the_requested_notification() {
        let mut notifications = Notifications::new();
        let _ = notifications.success("First", "First body");
        let first = notifications.toasts[0].id;
        let _ = notifications.failure("Second", "Second body");
        let second = notifications.toasts[1].id;

        let _ = notifications.update(Message::Dismiss(first));

        assert_eq!(notifications.toasts.len(), 1);
        assert_eq!(notifications.toasts[0].id, second);
    }
}
