
use futures::FutureExt;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;
use log::{debug, error, info};

mod hpts;
use hpts::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  
    let socks: SocketAddr = "127.0.0.1:7778".parse().unwrap();

    let config = Arc::new(HptsConfig { socks5_addr: socks });

    let port: u16 = "7780".parse().unwrap();
    let http_proxy_sock = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), port);


    info!("http server listening on port {}", port);

    let mut listener = TcpListener::bind(http_proxy_sock).await?;
    loop {
        let (socket, _addr) = listener.accept().await?;
        debug!("accept from client: {}", _addr);
        let ctx = HptsContext::new(config.clone(), socket);
        let task = hpts_bridge(ctx).map(|r| {
            if let Err(e) = r {
                error!("{}", e);
            }
        });
        tokio::spawn(task);
    }
}
