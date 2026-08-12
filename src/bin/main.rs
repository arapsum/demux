use demux::{App, Result, app::Cli};

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();
    Cli::new().run(&mut app).await
}
