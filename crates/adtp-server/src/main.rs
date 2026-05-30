use adtp_server::{config::ServerConfig, daemon::ServerDaemon};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("adtp=info,warn")
        .init();

    let config = ServerConfig::load()?;
    let daemon = ServerDaemon::new(config);
    daemon.run().await?;

    Ok(())
}
