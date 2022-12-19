use std::{env, thread, net::TcpStream, io::{Write, Read}, time::Duration, fs::{self, OpenOptions}};
use tokio;


fn main(){
    // let mut p =  env::current_dir().unwrap().to_str().unwrap();
    let _file = OpenOptions::new()
        .write(true)
        .truncate(true) // 清空文件
        .open("/Users/xiewenyu/Desktop/rust-project/leaf/logs.log");
    
    // _ = fs::remove_file("/Users/xiewenyu/Desktop/rust-project/leaf/logs.log");
    // "output": "/Users/xiewenyu/Desktop/rust-project/leaf/logs.log"

    let config = r#"
    {
        "log": {
            "level": "debug"
        },
        "dns":{
            "servers":["1.1.1.1","8.8.8.8","114.114.114.114"]
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
                "protocol": "direct",
                "tag": "direct"
            },
            {
                "protocol": "failover",
                "settings": {
                    "actors": [
                        "ss1"
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
                    "address": "60.12.124.214",
            "port": 39807,
            "password": "jufR4G3YG1zACQ08",
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
            }
        ],
        "router":{
            "domainResolve": true ,
            "rules": [
                {
                    "domainKeyword": [
                        "ipinfo",
                        "iqiyi",
                        "qy"
                    ],
                    "target": "failover_out"
                },
                {
                    "ip": [
                        "114.114.114.114"
                    ],
                    "target": "failover_out"
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
    if let Err(e) = leaf::start(0, opts) {
        panic!("{}",e);
    }


    print!("end")

}