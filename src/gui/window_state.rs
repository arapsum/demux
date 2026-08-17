use iced::{Point, Size};

use crate::app::preferences::{WindowGeometry, WindowPreferences};

pub const DEFAULT_WINDOW_SIZE: Size = Size::new(1_180.0, 900.0);
pub const MIN_WINDOW_SIZE: Size = Size::new(860.0, 720.0);

/// Tracks live window geometry and the persisted geometry policy.
#[derive(Debug)]
pub struct WindowState {
    preferences: WindowPreferences,
    size: Size,
    position: Option<Point>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            preferences: WindowPreferences::default(),
            size: DEFAULT_WINDOW_SIZE,
            position: None,
        }
    }
}

impl WindowState {
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn apply_preferences(&mut self, preferences: WindowPreferences) {
        self.preferences = preferences;
        if let Some(geometry) = preferences.geometry {
            self.size = sanitize_size(geometry.width, geometry.height);
            self.position = geometry
                .x
                .zip(geometry.y)
                .map(|(x, y)| Point::new(x as f32, y as f32));
        }
    }

    pub(crate) const fn set_remember_geometry(&mut self, remember: bool) {
        self.preferences.remember_geometry = remember;
        if !remember {
            self.preferences.geometry = None;
        }
    }

    pub(crate) const fn remember_geometry(&self) -> bool {
        self.preferences.remember_geometry
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) const fn resized(&mut self, size: Size) {
        self.size = sanitize_size(size.width as u32, size.height as u32);
    }

    pub(crate) const fn moved(&mut self, position: Point) {
        self.position = Some(position);
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub(crate) fn preferences(&self) -> WindowPreferences {
        let geometry = self.preferences.remember_geometry.then(|| WindowGeometry {
            width: self.size.width.round() as u32,
            height: self.size.height.round() as u32,
            x: self.position.map(|position| position.x.round() as i32),
            y: self.position.map(|position| position.y.round() as i32),
        });

        WindowPreferences {
            remember_geometry: self.preferences.remember_geometry,
            geometry,
        }
    }

    pub(crate) const fn size(&self) -> Size {
        self.size
    }

    pub(crate) const fn position(&self) -> Option<Point> {
        self.position
    }
}

#[allow(clippy::cast_precision_loss)]
const fn sanitize_size(width: u32, height: u32) -> Size {
    Size::new(
        (width as f32).max(MIN_WINDOW_SIZE.width),
        (height as f32).max(MIN_WINDOW_SIZE.height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_geometry_is_clamped_to_the_supported_minimum() {
        let mut state = WindowState::default();
        state.apply_preferences(WindowPreferences {
            remember_geometry: true,
            geometry: Some(WindowGeometry {
                width: 100,
                height: 200,
                x: Some(20),
                y: Some(30),
            }),
        });

        assert_eq!(state.size(), MIN_WINDOW_SIZE);
        assert_eq!(state.position(), Some(Point::new(20.0, 30.0)));
    }

    #[test]
    fn disabling_geometry_retention_clears_the_saved_value() {
        let mut state = WindowState::default();
        state.apply_preferences(WindowPreferences {
            remember_geometry: true,
            geometry: Some(WindowGeometry {
                width: 1_180,
                height: 900,
                x: None,
                y: None,
            }),
        });
        state.set_remember_geometry(false);

        assert_eq!(
            state.preferences(),
            WindowPreferences {
                remember_geometry: false,
                geometry: None,
            }
        );
    }
}
