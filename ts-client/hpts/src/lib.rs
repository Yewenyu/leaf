
use futures::FutureExt;
use serde::{Serialize, Deserialize, Deserializer};
use core::time;
use std::error::Error;
use std::net::{SocketAddr};
use std::ops::Add;
use std::sync::Arc;
use std::thread;
use tokio::net::TcpListener;
use log::{debug, error, info};

mod hpts;
use hpts::*;

#[tokio::main]
pub async fn start(http_addr:String,socks_addr:String) -> Result<(), Box<dyn Error>> {
  
    let socks: SocketAddr = socks_addr.parse().unwrap();

    let config = Arc::new(HptsConfig { socks5_addr: socks });

    let http_proxy_sock: SocketAddr = http_addr.parse().unwrap();


    info!("http server listening on  {}", http_proxy_sock);

    let listener = TcpListener::bind(http_proxy_sock).await?;
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

use serde_json::Value;
#[derive(Serialize, Deserialize, Debug)]
struct Addrs {
    http: String,
    socks: String,
}
pub fn start_with_json(json:String){
    let mut object: Value = serde_json::from_str(&json).unwrap();
    
    if let Some(value) = object.get_mut("http") {
        let v : Addrs = serde_json::from_str(value.to_string().as_str()).unwrap();
        
        thread::spawn(move||{
//             let ten_millis = time::Duration::from_millis(1000);
// // let now = time::Instant::now();

//             thread::sleep(ten_millis);
            _ = start(v.http, v.socks);
        });
    }
}