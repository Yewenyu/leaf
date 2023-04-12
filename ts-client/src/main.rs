use std::{env, thread, net::TcpStream, io::{Write, Read}, time::Duration, fs::{self, OpenOptions}, error::Error, collections::HashMap};

fn main(){
    // let mut p =  env::current_dir().unwrap().to_str().unwrap();
    let _file = OpenOptions::new()
        .write(true)
        .truncate(true) // 清空文件
        .open("/Users/xiewenyu/Desktop/rust-project/leaf/logs.log");

    //     doh("www.google.com.hk".to_string());
    // udp_example();
    // _ = fs::remove_file("/Users/xiewenyu/Desktop/rust-project/leaf/logs.log");
    // "output": "/Users/xiewenyu/Desktop/rust-project/leaf/logs.log"
    let config = r#"
    {
        "log": {
            "level": "debug"
        },
        "dns":{
            "servers":["1.1.1.1"],
            "dohkeys":{"1.1.1.1":"auto"}
        },
        "http":{
            "http":"127.0.0.1:7779",
            "socks":"127.0.0.1:7778"
        },
        "inbounds": [
            {
                "protocol": "socks",
                "address": "127.0.0.1",
                "port": 7778
            },
            {
                "protocol": "tun",
                "address": "0.0.0.0",
                "port": 9998,
                "settings": {
                    "fd": 1
                },
                "fakeDnsInclude":["google.com","gstatic.com"]
            }
        ],
        "outbounds": [
            {
                "protocol": "failover",
                "settings": {
                    "actors": [
                        "ss"
                    ],
                    "failTimeout":2,
                    "healthCheck":true,
                    "healthCheckTimeout":5,
                    "checkInterval":3,
                    "healthCheckActive":3,
                    "healthCheckAddr":"captive.apple.com:80",
                    "healthCheckContent":"HEAD / HTTP/1.1\r\n\r\n",
                    "failover":true
                },
                "tag": "failover_out"
            },
            {
                "protocol": "shadowsocks",
                "settings": {
                    "address": "51.79.157.25",
            "port": 35039,
            "password": "0qg09j1tAbzh3sT1",
            "method": "aes-256-gcm"
                },
                "tag":"ss"
            },
            {
                "protocol": "shadowsocks",
                "settings": {
                    "address": "127.0.0.1",
                    "port": 6669,
                    "password": "111111",
                    "method": "aes-256-gcm"
                },
                "tag":"ss1"
            },
            {
                "protocol": "direct",
                "tag": "direct"
            }
        ],
        "router":{
            "domainResolve": true ,
            "rules": [
                {
                    "geoip": [
                        "cn"
                    ],
                    "geoPath": "/Users/xiewenyu/Desktop/rust-project/leaf/ts-client/src/geo.mmdb",
                    "target": "direct"
                },
                {
                    "domainKeyword": [
                        "google"
                    ],
                    "target": "direct"
                }
            ]
        }
        
    }
    "#;

    // thread::spawn(||{
        

    //     loop {
    //         thread::sleep(Duration::from_secs(1));
    //         let mut stream = TcpStream::connect("127.0.0.1:5555").expect("connect failed");
    //         stream
    //             .write(b"ping")
    //             .expect("write failed");
    //         let mut buf = *b"1111";
    //         stream.read_exact(&mut buf);
    //         println!("{:?}",String::from_utf8(buf.to_vec()));
    
    //     }
    // });
    

    let opts = leaf::StartOptions {
        config: leaf::Config::Str(config.to_string()),
        auto_reload: false,
        runtime_opt: leaf::RuntimeOption::MultiThreadAuto(2097152),
    };
    hpts::start_with_json(config.to_string());
    if let Err(e) = leaf::start(0, opts) {
        panic!("{}",e);
    }

    
    print!("end")

}

use rustdns::Message;
use rustdns::types::*;
use std::net::UdpSocket;

fn udp_example() -> std::io::Result<()> {
    // A DNS Message can be easily constructed
    let mut m = Message::default();
    m.add_question("www.google.com.hk", Type::A, Class::Internet);
    m.add_extension(Extension {   // Optionally add a EDNS extension
        payload_size: 4096,       // which supports a larger payload size.
        ..Default::default()
    });

    // Setup a UDP socket for sending to a DNS server.
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    socket.set_read_timeout(Some(Duration::new(5, 0)))?;
    socket.connect("8.8.8.8:53")?; // Google's Public DNS Servers

    // Encode the DNS Message as a Vec<u8>.
    let question = m.to_vec()?;

    // Send to the server.
    socket.send(&question)?;

    // Wait for a response from the DNS server.
    let mut resp = [0; 4096];
    let len = socket.recv(&mut resp)?;
    // Take the response bytes and turn it into another DNS Message.
    let answer = Message::from_slice(&resp[0..len])?;
    println!("DNS Response:\n{}", answer);
    let len = socket.recv(&mut resp)?;
    // Take the response bytes and turn it into another DNS Message.
    let answer = Message::from_slice(&resp[0..len])?;

    // Now do something with `answer`, in this case print it!
    println!("DNS Response:\n{}", answer);

    Ok(())
}

use reqwest::{Url};
use serde_json::Value;

fn doh(host:String) -> Result<Vec<String>, Box<dyn Error>>{
    let url = Url::parse(format!("https://cloudflare-dns.com/dns-query?name={}",host).as_str())?;
    
    let client = reqwest::blocking::Client::new();

    let mut response = client.get(url)
    .header("accept", "application/dns-json")
    .send()?;

    let result = response.json::<HashMap<String, Value>>()?;
    
    let answers = result.get("Answer").map(|v|{
        v.as_array()
    });

    if let Some(Some(answers)) = answers {
        let v:Vec<String> = answers.iter().map(|v|{
            if let Some(Some(Some(v))) = v.as_object().map(|v|{v.get("data").map(|v|{v.as_str()})}) {
                return v.to_string();
            }
            return "".to_string();
        }).collect();

        return Ok(v);
    }

let v : Vec<String> = Vec::new();
return Ok(v)
}