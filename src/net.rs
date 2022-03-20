use netaddr2::{Broadcast, NetAddr};
use std::collections::HashMap;
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::sync::Mutex;

use crate::protocol::{AnnounceMessage, MessageType, QueryMessage};
use crate::Packet;

pub async fn monitor() -> io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:11430").await?;
    let mut buf = [0; 1024];
    loop {
        let (len, addr) = sock.recv_from(&mut buf).await?;

        let p = crate::Packet::decode(&buf[..len]).unwrap();
        dbg!(p);
        println!("{:?} bytes received from {:?}", len, addr);
    }
}

pub struct Discover {
    query_channel: mpsc::Sender<QueryMessage>,
}

impl Discover {
    pub async fn start() -> Result<Discover, DiscoverError> {
        let socket = UdpSocket::bind("0.0.0.0:11430").await?;
        socket.set_broadcast(true)?;

        let (qtx, rx) = mpsc::channel(100);

        tokio::spawn(Discover::process(socket, rx));

        Ok(Discover { query_channel: qtx })
    }

    pub async fn query(
        &self,
        q: QueryMessage,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<QueryMessage>> {
        self.query_channel.send(q).await
    }

    async fn process(
        socket: UdpSocket,
        mut query: mpsc::Receiver<QueryMessage>,
    ) -> Result<(), DiscoverError> {
        loop {
            let mut buf = [0; 1024];

            tokio::select! {
                val = socket.recv_from(&mut buf) => {
                    let (len, addr) = val?;
                    println!("{:?} bytes received from {:?}", len, addr);
                }

                val = query.recv() => {
                    match val {
                        Some(q) => {
                            Discover::broadcast_query(&socket, q).await;
                        },
                        None => {}
                    }
                }
            }
        }
    }

    async fn broadcast_query(socket: &UdpSocket, q: QueryMessage) -> Result<(), DiscoverError> {
        let s = socket.local_addr()?;

        match s.ip() {
            IpAddr::V4(ip) => {
                let net: NetAddr = ip.into();
                let ok = net.broadcast();
                dbg!(ok);
            }
            IpAddr::V6(ip) => return Err(DiscoverError::IPv6NotSupported),
        }

        // Create a new packet with the query
        let p = Packet::new(MessageType::BroadcastQuery(q)).bytes();

        // Broadcast the query
        let _ = socket.send_to(&p, "0.0.0.0:11430").await;

        Ok(())
    }

    // async fn process(socket: UdpSocket) -> Result<(), DiscoverError> {
    //     let mut m: HashMap<Vec<u8>, AnnounceMessage> = HashMap::new();

    //     loop {
    //         let mut buf = [0; 1024];
    //         let (len, addr) = socket.recv_from(&mut buf).await?;

    //         // let p = crate::Packet::decode(&buf[..len]).unwrap();

    //         match crate::Packet::decode(&buf[..len]) {
    //             Ok(v) => match v.message {
    //                 MessageType::Announce(p) => {
    //                     m.insert(p.node_id.to_vec(), p);
    //                 }
    //                 _ => {}
    //             },
    //             Err(e) => {}
    //         }

    //         println!("{:?} bytes received from {:?}", len, addr);
    //     }
    // }
}

#[derive(Error, Debug)]
pub enum DiscoverError {
    #[error(transparent)]
    NetworkError(#[from] std::io::Error),

    #[error("IPv6 is not supported")]
    IPv6NotSupported,
}
