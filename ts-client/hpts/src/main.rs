
use futures::FutureExt;
use std::error::Error;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::net::TcpListener;
use log::{debug, error, info};



#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
  
   return hpts::start("127.0.0.1:7780".to_string(), "127.0.0.1:7778".to_string());
    
}
