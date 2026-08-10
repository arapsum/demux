use demux::{App, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let mut app = App::new();
    app.run().await
}
