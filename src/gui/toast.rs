use iced::font::Weight;
use iced::widget::{button, column, container, row, space, stack, text};
use iced::{Border, Color, Element, Fill, Padding, Theme};

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

pub(crate) fn overlay<'a>(
    content: impl Into<Element<'a, Message>>,
    toasts: &'a [Toast],
) -> Element<'a, Message> {
    let notices = toasts.iter().rev().take(3).fold(
        column![].spacing(10).align_x(iced::Alignment::End),
        |notices, toast| notices.push(toast_view(toast)),
    );

    let layer = container(notices)
        .padding(Padding::from([24, 26]))
        .align_right(Fill)
        .align_top(Fill);

    stack![content.into(), layer].into()
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
