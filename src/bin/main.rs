use demux::{App, Result, app::Cli, telemetry};

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "Starting Demux");

    let mut app = App::new();
    let result = Cli::new().run(&mut app).await;

    if let Err(e) = &result {
        tracing::error!(error = %e, "application terminated with an error");
    }

    result
}
