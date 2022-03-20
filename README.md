# LSDP (Lenbrook Service Discovery Protocol) in Rust

You've probably never heard about this protocol. Neither had I before implementing it. [Lenbrook Group](https://lenbrook.com/) is the manufacturer behind brands like Bluesound, NAD etc. and have in their infinte wisdom decided that instead of leveraging the multitude of avaliable service discovery protocols out there, they would write their own.

Now why would a manufacturer forego using established standards like mDNS or SSDP? Lenbrook claims [it's because their customers lack the ability to configure their networks](https://nadelectronics.com/wp-content/uploads/2020/12/Custom-Integration-API-v1.0_Dec_2020.pdf). Maybe if their customer didn't buy "audiophile" garbage routers this would be less of a problem but here we are.

The protocol is basically a worse version of mDNS over UDP Broadcast instead of UDP Multicast. In an effort to get some more experience with parsing binary protocols in Rust I wrote this crate mostly to explore [Nom](https://github.com/Geal/nom). The crate ships with tokio features turned on which facilites service discovery but if you for some reason just want the decoding, you can disable the tokio features which gives you a reasonably lightweight crate.

### How do I use this?

```rust
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
```