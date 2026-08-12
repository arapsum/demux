use iced::widget::Text;
use iced::{Color, Theme};
use lucide_icons::iced::{icon_file_plus_2, icon_file_video_2, icon_folder_plus, icon_trash_2};

pub(crate) fn add_files(color: Color) -> Text<'static, Theme> {
    icon_file_plus_2().size(17).color(color)
}

pub(crate) fn add_folder(color: Color) -> Text<'static, Theme> {
    icon_folder_plus().size(17).color(color)
}

pub(crate) fn remove(color: Color) -> Text<'static, Theme> {
    icon_trash_2().size(16).color(color)
}

pub(crate) fn media_file(color: Color) -> Text<'static, Theme> {
    icon_file_video_2().size(42).color(color)
}

pub(crate) fn queue_media(color: Color) -> Text<'static, Theme> {
    icon_file_video_2().size(18).color(color)
}
