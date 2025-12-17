use tokio::io;
use tokio::net::TcpListener;
#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6663").await.unwrap();

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        tokio::spawn(async move {
            let (mut reader, mut writer) = socket.split();
            io::copy(&mut reader, &mut writer).await.unwrap();
        });
    }
}
