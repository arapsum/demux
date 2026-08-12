use demux::{gui, telemetry};

fn main() -> iced::Result {
    telemetry::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting Demux");

    let result = gui::run();

    if let Err(e) = &result {
        tracing::error!(error = %e, "application terminated with an error");
    }

    result
}
