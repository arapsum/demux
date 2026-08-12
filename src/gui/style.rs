use iced::widget::container;
use iced::{Border, Color, Shadow, Theme};

pub(crate) const TEXT_MUTED: Color = Color::from_rgb(0.62, 0.64, 0.70);
pub(crate) const ACCENT: Color = Color::from_rgb(0.43, 0.36, 0.96);
pub(crate) const SUCCESS: Color = Color::from_rgb(0.35, 0.78, 0.57);
pub(crate) const WARNING: Color = Color::from_rgb(0.96, 0.68, 0.30);
pub(crate) const DANGER: Color = Color::from_rgb(0.94, 0.39, 0.42);
pub(crate) const DANGER_TEXT: Color = Color::from_rgb(0.95, 0.76, 0.77);

pub(crate) fn app_background(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgb(0.055, 0.06, 0.075))
}

pub(crate) fn panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.085, 0.092, 0.112))
        .border(Border {
            color: Color::from_rgb(0.17, 0.18, 0.22),
            width: 1.0,
            radius: 14.0.into(),
        })
        .shadow(Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: iced::Vector::new(0.0, 5.0),
            blur_radius: 18.0,
        })
}

pub(crate) fn inset_panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.065, 0.07, 0.087))
        .border(Border {
            color: Color::from_rgb(0.19, 0.20, 0.24),
            width: 1.0,
            radius: 12.0.into(),
        })
}

pub(crate) fn selected_row(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.075, 0.08, 0.10))
        .border(Border {
            color: Color::from_rgb(0.30, 0.27, 0.55),
            width: 1.0,
            radius: 12.0.into(),
        })
}

pub(crate) fn accent_tile(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.20, 0.17, 0.43))
        .border(Border {
            color: Color::from_rgb(0.34, 0.29, 0.67),
            width: 1.0,
            radius: 12.0.into(),
        })
}

pub(crate) fn error_panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.16, 0.075, 0.085))
        .border(Border {
            color: Color::from_rgb(0.38, 0.14, 0.16),
            width: 1.0,
            radius: 12.0.into(),
        })
}
