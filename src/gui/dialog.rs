use iced::widget::{container, mouse_area, opaque, space, stack};
use iced::{Element, Fill};

use super::style::dialog_backdrop;

pub fn modal<'a, Message: Clone + 'a>(
    content: Element<'a, Message>,
    dialog: Element<'a, Message>,
    dismiss: Option<Message>,
) -> Element<'a, Message> {
    let backdrop = container(space::vertical())
        .width(Fill)
        .height(Fill)
        .style(dialog_backdrop);
    let backdrop: Element<'a, Message> = match dismiss {
        Some(message) => mouse_area(backdrop).on_press(message).into(),
        None => backdrop.into(),
    };
    // Keep the centering container transparent to mouse events outside the
    // dialog card. Only the card itself should stop events reaching the
    // dismissible backdrop beneath it.
    let dialog = container(opaque(dialog))
        .width(Fill)
        .height(Fill)
        .center_x(Fill)
        .center_y(Fill);

    stack![content, opaque(backdrop), dialog].into()
}
