use default_net;
use netaddr2::{Broadcast, NetAddr};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::protocol::{AnnounceMessage, MessageType, QueryMessage};
use crate::Packet;

pub async fn monitor(c: mpsc::Sender<Packet>) -> Result<(), DiscoverError> {
    let sock = UdpSocket::bind("0.0.0.0:11430").await?;
    let mut buf = [0; 1024];
    loop {
        let (len, _) = sock.recv_from(&mut buf).await?;

        let p = match crate::Packet::decode(&buf[..len]) {
            Ok(v) => v,
            Err(_) => return Err(DiscoverError::ParseError),
        };

        c.send(p).await?;
    }
}

pub struct Discover {
    query_channel: mpsc::Sender<QueryMessage>,
    index: Arc<Mutex<HashMap<Vec<u8>, AnnounceMessage>>>,
    updates: broadcast::Sender<Packet>,

    cancel_process: mpsc::Sender<()>,
    cancel_index: mpsc::Sender<()>,
}

impl Discover {
    pub async fn start() -> Result<Discover, DiscoverError> {
        let socket = UdpSocket::bind("0.0.0.0:11430").await?;
        socket.set_broadcast(true)?;

        let index = Arc::new(Mutex::new(HashMap::new()));

        let (qtx, rx) = mpsc::channel(100);

        let (discovered, _) = broadcast::channel(100);

        let (ctx, crx) = mpsc::channel(1);
        let (itx, irx) = mpsc::channel(1);

        tokio::spawn(Discover::index(discovered.subscribe(), index.clone(), irx));
        tokio::spawn(Discover::process(socket, rx, discovered.clone(), crx));

        Ok(Discover {
            query_channel: qtx,
            index,
            updates: discovered,
            cancel_process: ctx,
            cancel_index: itx,
        })
    }

    pub async fn query(
        &self,
        q: QueryMessage,
    ) -> Result<(), tokio::sync::mpsc::error::SendError<QueryMessage>> {
        self.query_channel.send(q).await
    }

    pub async fn inventory(&self) -> Arc<Mutex<HashMap<Vec<u8>, AnnounceMessage>>> {
        self.index.clone()
    }

    pub async fn updates(&self) -> broadcast::Sender<Packet> {
        self.updates.clone()
    }

    async fn index(
        mut ch: broadcast::Receiver<Packet>,
        d: Arc<Mutex<HashMap<Vec<u8>, AnnounceMessage>>>,
        mut cancel: mpsc::Receiver<()>,
    ) -> Result<(), DiscoverError> {
        loop {
            tokio::select! {
                p = ch.recv() => {
                    match p?.message {
                        MessageType::Announce(v) => {
                            d.lock().await.insert(v.node_id.clone(), v);
                        }
                        _ => {}
                    }
                }

                _ =  cancel.recv() => {
                    return Ok(())
                }
            }
        }
    }

    async fn process(
        socket: UdpSocket,
        mut query: mpsc::Receiver<QueryMessage>,
        discovered: broadcast::Sender<Packet>,
        mut cancel: mpsc::Receiver<()>,
    ) -> Result<(), DiscoverError> {
        let mut buf = [0; 1024];
        loop {
            tokio::select! {
                val = socket.recv_from(&mut buf) => {
                    let (len, _) = val?;
                    let p = crate::Packet::decode(&buf[..len]).unwrap();
                    discovered.send(p)?;
                }

                val = query.recv() => {
                    match val {
                        Some(q) => {
                            Discover::broadcast_query(&socket, q).await?;
                        },
                        None => {}
                    }
                }

                _ =  cancel.recv() => {
                    return Ok(())
                }
            }
        }
    }

    async fn broadcast_query(socket: &UdpSocket, q: QueryMessage) -> Result<(), DiscoverError> {
        // Create a new packet with the query
        let p = Packet::new(MessageType::BroadcastQuery(q)).bytes();

        let addr = Discover::get_broadcast_address()?;

        // Broadcast the query
        let _ = socket.send_to(&p, SocketAddr::new(addr, 11430)).await;

        Ok(())
    }

    fn get_broadcast_address() -> Result<IpAddr, DiscoverError> {
        match default_net::get_default_interface() {
            Ok(default_interface) => {
                for iface in default_interface.ipv4 {
                    let v = format!("{}/{}", iface.addr, iface.prefix_len);
                    let net: NetAddr = v.parse().unwrap();
                    match net.broadcast() {
                        Some(v) => return Ok(v),
                        None => {}
                    }
                }
                Err(DiscoverError::CouldNotFindBroadcastInterface)
            }
            Err(e) => Err(DiscoverError::InterfaceError(e)),
        }
    }
}

impl Drop for Discover {
    fn drop(&mut self) {
        let _ = self.cancel_process.try_send(());
        let _ = self.cancel_index.try_send(());
    }
}

#[derive(Error, Debug)]
pub enum DiscoverError {
    #[error(transparent)]
    NetworkError(#[from] std::io::Error),

    #[error("Interface Error {0}")]
    InterfaceError(String),

    #[error("IPv6 is not supported")]
    IPv6NotSupported,

    #[error("Could not find suitable broadcast interface")]
    CouldNotFindBroadcastInterface,

    #[error(transparent)]
    SendError(#[from] tokio::sync::broadcast::error::SendError<Packet>),

    #[error(transparent)]
    SendMPSCError(#[from] tokio::sync::mpsc::error::SendError<Packet>),

    #[error(transparent)]
    BroadcastError(#[from] tokio::sync::broadcast::error::RecvError),

    #[error("Parsing Error")]
    ParseError,
}
