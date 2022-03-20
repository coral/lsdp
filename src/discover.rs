use std::io;
use tokio::net::UdpSocket;

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
