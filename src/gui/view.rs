use iced::{
    Color, Element, Fill, FillPortion, Font, Padding,
    font::Weight,
    widget::{column, container, row, text},
};

use super::{
    message::Message,
    state::Demux,
    style::{TEXT_MUTED, accent_tile, app_background},
};

impl Demux {
    pub fn view(&self) -> Element<'_, Message> {
        let header = row![
            container(
                text("D")
                    .size(22)
                    .font(Font {
                        weight: Weight::Bold,
                        ..Font::default()
                    })
                    .color(Color::WHITE)
            )
            .width(48)
            .height(48)
            .center(48)
            .style(accent_tile),
            column![
                text("Demux").size(28).font(Font {
                    weight: Weight::Bold,
                    ..Font::default()
                }),
                text("Extract clean audio from video with FFmpeg")
                    .size(14)
                    .color(TEXT_MUTED),
            ]
            .spacing(3)
        ]
        .spacing(14)
        .align_y(iced::Alignment::Center);

        let work_area = column![
            self.queue.view(self.error.as_deref()).map(Message::Queue),
            self.progress.view().map(Message::Progress),
        ]
        .spacing(10)
        .width(FillPortion(7))
        .height(Fill);

        let workspace = row![work_area, self.settings_panel()]
            .spacing(16)
            .height(Fill);

        let content = container(column![header, workspace].spacing(18))
            .width(Fill)
            .height(Fill)
            .padding(Padding::from([24, 26]))
            .style(app_background);

        self.notifications.view(content)
    }

    fn settings_panel(&self) -> Element<'_, Message> {
        self.output_settings
            .view(
                &self.dependency_state,
                self.queue.selected_status(),
                self.queue.selected(),
                self.queue.run_progress(),
                self.can_start(),
                self.queue.has_folder_hierarchy(),
            )
            .map(Message::OutputSettings)
    }
}
