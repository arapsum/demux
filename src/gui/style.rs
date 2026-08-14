use iced::widget::{button, container, pick_list};
use iced::{Background, Border, Color, Shadow, Theme};

pub const TEXT_MUTED: Color = Color::from_rgb(0.62, 0.64, 0.70);
pub const ACCENT: Color = Color::from_rgb(0.43, 0.36, 0.96);
pub const SUCCESS: Color = Color::from_rgb(0.35, 0.78, 0.57);
pub const WARNING: Color = Color::from_rgb(0.96, 0.68, 0.30);
pub const DANGER: Color = Color::from_rgb(0.94, 0.39, 0.42);
pub const DANGER_TEXT: Color = Color::from_rgb(0.95, 0.76, 0.77);
pub const INSET_BACKGROUND: Color = Color::from_rgb(0.065, 0.07, 0.087);
pub const DROP_BORDER: Color = Color::from_rgb(0.31, 0.32, 0.38);
pub const ICON_MUTED: Color = Color::from_rgb(0.48, 0.50, 0.57);
pub const BUTTON_TEXT: Color = Color::from_rgb(0.94, 0.95, 0.98);

pub fn app_background(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgb(0.055, 0.06, 0.075))
}

pub fn panel(_theme: &Theme) -> container::Style {
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

pub fn inset_panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(INSET_BACKGROUND)
        .border(Border {
            color: Color::from_rgb(0.19, 0.20, 0.24),
            width: 1.0,
            radius: 12.0.into(),
        })
}

pub fn queue_header(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgb(0.075, 0.08, 0.098))
}

pub fn queue_row(_theme: &Theme) -> container::Style {
    container::Style::default()
}

pub fn selected_queue_row(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgb(0.105, 0.10, 0.16))
}

pub fn queue_footer(_theme: &Theme) -> container::Style {
    container::Style::default().background(Color::from_rgb(0.075, 0.08, 0.098))
}

pub fn accent_tile(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.20, 0.17, 0.43))
        .border(Border {
            color: Color::from_rgb(0.34, 0.29, 0.67),
            width: 1.0,
            radius: 12.0.into(),
        })
}

pub fn error_panel(_theme: &Theme) -> container::Style {
    container::Style::default()
        .background(Color::from_rgb(0.16, 0.075, 0.085))
        .border(Border {
            color: Color::from_rgb(0.38, 0.14, 0.16),
            width: 1.0,
            radius: 12.0.into(),
        })
}

pub fn settings_select(_theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let border_color = match status {
        pick_list::Status::Active => Color::from_rgb(0.19, 0.20, 0.24),
        pick_list::Status::Hovered | pick_list::Status::Opened { .. } => ACCENT,
    };

    pick_list::Style {
        text_color: BUTTON_TEXT,
        placeholder_color: TEXT_MUTED,
        handle_color: TEXT_MUTED,
        background: Background::Color(INSET_BACKGROUND),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 12.0.into(),
        },
    }
}

pub fn primary_action(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color, border_color) = match status {
        button::Status::Active => (ACCENT, Color::WHITE, ACCENT),
        button::Status::Hovered => (
            Color::from_rgb(0.50, 0.43, 1.0),
            Color::WHITE,
            Color::from_rgb(0.54, 0.47, 1.0),
        ),
        button::Status::Pressed => (
            Color::from_rgb(0.36, 0.29, 0.82),
            Color::WHITE,
            Color::from_rgb(0.40, 0.33, 0.90),
        ),
        button::Status::Disabled => (
            Color::from_rgb(0.16, 0.15, 0.28),
            ICON_MUTED,
            Color::from_rgb(0.20, 0.19, 0.34),
        ),
    };

    action_button(background, text_color, border_color)
}

pub fn secondary_action(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color, border_color) = match status {
        button::Status::Active => (
            Color::from_rgb(0.105, 0.112, 0.135),
            BUTTON_TEXT,
            Color::from_rgb(0.24, 0.25, 0.30),
        ),
        button::Status::Hovered => (
            Color::from_rgb(0.14, 0.15, 0.18),
            Color::WHITE,
            Color::from_rgb(0.32, 0.33, 0.39),
        ),
        button::Status::Pressed => (
            INSET_BACKGROUND,
            BUTTON_TEXT,
            Color::from_rgb(0.28, 0.29, 0.35),
        ),
        button::Status::Disabled => (
            Color::from_rgb(0.075, 0.08, 0.095),
            ICON_MUTED,
            Color::from_rgb(0.15, 0.16, 0.19),
        ),
    };

    action_button(background, text_color, border_color)
}

pub fn destructive_action(_theme: &Theme, status: button::Status) -> button::Style {
    let (background, text_color, border_color) = match status {
        button::Status::Active => (
            Color::from_rgb(0.11, 0.085, 0.10),
            DANGER_TEXT,
            Color::from_rgb(0.36, 0.17, 0.20),
        ),
        button::Status::Hovered => (
            Color::from_rgb(0.18, 0.08, 0.095),
            Color::from_rgb(1.0, 0.83, 0.84),
            Color::from_rgb(0.60, 0.24, 0.27),
        ),
        button::Status::Pressed => (Color::from_rgb(0.22, 0.075, 0.09), Color::WHITE, DANGER),
        button::Status::Disabled => (
            Color::from_rgb(0.075, 0.08, 0.095),
            ICON_MUTED,
            Color::from_rgb(0.15, 0.16, 0.19),
        ),
    };

    action_button(background, text_color, border_color)
}

pub fn queue_row_action(_theme: &Theme, status: button::Status) -> button::Style {
    let background = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgb(0.105, 0.11, 0.135))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgb(0.075, 0.08, 0.098))),
        button::Status::Active | button::Status::Disabled => None,
    };

    button::Style {
        background,
        text_color: BUTTON_TEXT,
        border: Border::default(),
        shadow: Shadow::default(),
        snap: true,
    }
}

fn action_button(background: Color, text_color: Color, border_color: Color) -> button::Style {
    button::Style {
        background: Some(Background::Color(background)),
        text_color,
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 7.0.into(),
        },
        shadow: Shadow::default(),
        snap: true,
    }
}
