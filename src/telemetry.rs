use tracing_subscriber::EnvFilter;

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("demux=info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        // TODO: turn target off in production
        .with_target(true)
        // TODO: may need to redirect to a log file in production
        .with_writer(std::io::stderr)
        .init();
}
