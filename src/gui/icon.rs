use iced::widget::Text;
use iced::{Color, Theme};
use lucide_icons::iced::{
    icon_download, icon_external_link, icon_file_plus_2, icon_file_video_2, icon_folder_plus,
    icon_info, icon_pause, icon_play, icon_refresh_cw, icon_settings, icon_square, icon_trash_2,
    icon_x,
};

pub fn add_files(color: Color) -> Text<'static, Theme> {
    icon_file_plus_2().size(17).color(color)
}

pub fn add_folder(color: Color) -> Text<'static, Theme> {
    icon_folder_plus().size(17).color(color)
}

pub fn remove(color: Color) -> Text<'static, Theme> {
    icon_trash_2().size(16).color(color)
}

pub fn save(color: Color) -> Text<'static, Theme> {
    icon_download().size(16).color(color)
}

pub fn media_file(color: Color) -> Text<'static, Theme> {
    icon_file_video_2().size(42).color(color)
}

pub fn queue_media(color: Color) -> Text<'static, Theme> {
    icon_file_video_2().size(18).color(color)
}

pub fn stop(color: Color) -> Text<'static, Theme> {
    icon_square().size(14).color(color)
}

pub fn pause(color: Color) -> Text<'static, Theme> {
    icon_pause().size(14).color(color)
}

pub fn play(color: Color) -> Text<'static, Theme> {
    icon_play().size(14).color(color)
}

pub fn settings(color: Color) -> Text<'static, Theme> {
    icon_settings().size(16).color(color)
}

pub fn info(color: Color) -> Text<'static, Theme> {
    icon_info().size(16).color(color)
}

pub fn refresh(color: Color) -> Text<'static, Theme> {
    icon_refresh_cw().size(15).color(color)
}

pub fn close(color: Color) -> Text<'static, Theme> {
    icon_x().size(15).color(color)
}

pub fn external_link(color: Color) -> Text<'static, Theme> {
    icon_external_link().size(14).color(color)
}
