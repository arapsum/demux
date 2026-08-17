use iced::{
    Element, Fill, FillPortion, Length, Padding,
    widget::{column, container, row, svg, text},
};

use super::{
    message::Message,
    state::Demux,
    style::{TEXT_MUTED, app_background},
};

impl Demux {
    pub fn view(&self) -> Element<'_, Message> {
        let header = column![
            svg(svg::Handle::from_memory(include_bytes!(
                "../../assets/demux-logo.svg"
            )))
            .width(Length::Fixed(280.0))
            .height(Length::Fixed(73.0)),
            text("Extract clean audio from video with FFmpeg")
                .size(14)
                .color(TEXT_MUTED),
        ]
        .spacing(2);

        let work_area = column![
            self.queue.view(self.error.as_deref()).map(Message::Queue),
            self.progress.view().map(Message::Progress),
        ]
        .spacing(10)
        .width(FillPortion(7))
        .height(Fill);

        let workspace = row![work_area, self.settings_panel()]
            .spacing(16)
            .height(FillPortion(3));

        let logs = container(self.logs.view().map(Message::Logs))
            .width(Fill)
            .height(FillPortion(1));

        let content = container(column![header, workspace, logs].spacing(18))
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
