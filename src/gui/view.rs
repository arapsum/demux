use iced::{
    Alignment, Element, Fill, FillPortion, Length, Padding,
    widget::{button, column, container, row, scrollable, space, svg, text},
};

use super::{
    about, application_settings, dialog, icon,
    message::Message,
    state::Demux,
    style::{BUTTON_TEXT, TEXT_MUTED, app_background, secondary_action},
};

const COMPACT_BREAKPOINT: f32 = 1_000.0;
const COMPACT_QUEUE_HEIGHT: f32 = 360.0;
const COMPACT_PROGRESS_HEIGHT: f32 = 255.0;
const COMPACT_SETTINGS_HEIGHT: f32 = 700.0;
const COMPACT_LOG_HEIGHT: f32 = 280.0;

impl Demux {
    pub fn view(&self) -> Element<'_, Message> {
        let compact = self.window.size().width < COMPACT_BREAKPOINT;
        let header = Self::header();

        let queue = self.queue.view(self.error.as_deref()).map(Message::Queue);
        let progress = self.progress.view(self.can_start()).map(Message::Progress);
        let settings = self.settings_panel(compact);
        let logs = self.logs.view().map(Message::Logs);

        let body: Element<'_, Message> = if compact {
            scrollable(
                column![
                    container(queue)
                        .width(Fill)
                        .height(Length::Fixed(COMPACT_QUEUE_HEIGHT)),
                    container(progress)
                        .width(Fill)
                        .height(Length::Fixed(COMPACT_PROGRESS_HEIGHT)),
                    container(settings)
                        .width(Fill)
                        .height(Length::Fixed(COMPACT_SETTINGS_HEIGHT)),
                    container(logs)
                        .width(Fill)
                        .height(Length::Fixed(COMPACT_LOG_HEIGHT)),
                ]
                .spacing(10),
            )
            .width(Fill)
            .height(Fill)
            .into()
        } else {
            let work_area = column![queue, progress]
                .spacing(10)
                .width(FillPortion(7))
                .height(Fill);
            let workspace = row![work_area, settings].spacing(16).height(FillPortion(3));
            let logs = container(logs).width(Fill).height(FillPortion(1));

            column![workspace, logs].spacing(10).into()
        };

        let content = container(column![header, body].spacing(18))
            .width(Fill)
            .height(Fill)
            .padding(Padding::from([24, 26]))
            .style(app_background);

        self.with_overlays(self.notifications.view(content))
    }

    fn header() -> Element<'static, Message> {
        let identity = column![
            svg(svg::Handle::from_memory(include_bytes!(
                "../../assets/demux-logo.svg"
            )))
            .width(Length::Fixed(230.0))
            .height(Length::Fixed(60.0)),
            text("Extract clean audio from video with FFmpeg")
                .size(14)
                .color(TEXT_MUTED),
        ]
        .spacing(2);

        row![
            identity,
            space::horizontal(),
            row![
                header_button(
                    icon::settings(BUTTON_TEXT),
                    "Settings",
                    Message::ApplicationSettings(application_settings::Message::Open),
                ),
                header_button(
                    icon::info(BUTTON_TEXT),
                    "About",
                    Message::About(about::Message::Open),
                ),
            ]
            .spacing(8),
        ]
        .align_y(Alignment::Center)
        .width(Fill)
        .into()
    }

    fn settings_panel(&self, compact: bool) -> Element<'_, Message> {
        self.output_settings
            .view(
                &self.dependency_state,
                self.queue.selected_status(),
                self.queue.selected(),
                self.queue.run_progress(),
                self.queue.has_folder_hierarchy(),
                compact,
            )
            .map(Message::OutputSettings)
    }

    fn with_overlays<'a>(&'a self, content: Element<'a, Message>) -> Element<'a, Message> {
        if let Some(dialog) = self.close_confirmation.view() {
            return dialog::modal(content, dialog.map(Message::CloseConfirmation), None);
        }

        if let Some(dialog) = self.application_settings.view(
            &self.dependency_state,
            self.window.remember_geometry(),
            self.queue.is_running(),
        ) {
            return dialog::modal(
                content,
                dialog.map(Message::ApplicationSettings),
                Some(Message::ApplicationSettings(
                    application_settings::Message::Close,
                )),
            );
        }

        if let Some(dialog) = self.about.view() {
            return dialog::modal(
                content,
                dialog.map(Message::About),
                Some(Message::About(about::Message::Close)),
            );
        }

        content
    }
}

fn header_button<'a>(
    icon: iced::widget::Text<'static, iced::Theme>,
    label: &'a str,
    message: Message,
) -> Element<'a, Message> {
    button(
        row![icon, text(label).size(13)]
            .spacing(7)
            .align_y(Alignment::Center),
    )
    .padding(Padding::from([9, 12]))
    .style(secondary_action)
    .on_press(message)
    .into()
}
