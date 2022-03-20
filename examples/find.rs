use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    lsdp::discover::monitor().await;
    Ok(())
}
