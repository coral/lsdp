use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    Ok(lsdp::net::monitor().await?)
}
