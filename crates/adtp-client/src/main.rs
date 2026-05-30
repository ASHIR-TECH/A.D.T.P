use adtp_client::{config::ClientConfig, daemon::ClientDaemon};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("adtp=info,warn")
        .init();

    let config = ClientConfig::load()?;
    let daemon = ClientDaemon::new(config)?;
    daemon.run().await?;

    Ok(())
}
