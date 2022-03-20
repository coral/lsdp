use anyhow::Result;
use lsdp::{net::Discover, ClassID};

#[tokio::main]
async fn main() -> Result<()> {
    let d = Discover::start().await?;

    d.query(lsdp::QueryMessage::new(vec![ClassID::All])).await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(100)).await;

    Ok(())
}
