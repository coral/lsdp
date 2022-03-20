use anyhow::Result;
use lsdp::{net::Discover, ClassID};

#[tokio::main]
async fn main() -> Result<()> {
    println!("Finding devices using LSDP");

    let d = Discover::start().await?;

    d.query(lsdp::QueryMessage::new(vec![ClassID::All])).await?;

    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

    for (_, d) in d.inventory().await.lock().await.iter() {
        println!(
            "Found {}: {:?} with data {:?}",
            d.addr, d.records[0].cid, d.records[0].data
        );
    }

    Ok(())
}
