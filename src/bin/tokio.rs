use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};

use tokio::{
    io::{self, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() {
    let addr1 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(111,63,65,103)), 80);
    let addr2 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(150,171,28,10)), 80);
    race(b"hello world", addr2, addr1).await.unwrap();
}

async fn race(data: &[u8], addr1: SocketAddr, addr2: SocketAddr) -> io::Result<()> {
    tokio::select! {
        Ok(_) = async {
            let mut socket = TcpStream::connect(addr1).await?;
            socket.write_all(data).await?;
            Ok::<_,io::Error>(())
        } => {
            println!("{}", addr1)
        }

        Ok(_) = async {
            let mut socket = TcpStream::connect(addr2).await?;
            socket.write_all(data).await?;
            Ok::<_,io::Error>(())
        } => {
            println!("{}", addr2)
        }

        else => {}
    };
    Ok(())
}
