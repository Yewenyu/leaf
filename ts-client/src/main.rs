use std::env;



fn main(){

    let config = r#"
    {
        "log": {
            "level": "debug"
        },
        "dns": {
            "servers": [
                "114.114.114.114",
                "1.1.1.1",
                "8.8.8.8"
            ]
        },
        "inbounds": [
            {
                "protocol": "socks",
                "address": "127.0.0.1",
                "port": 7778
            }
        ],
        "outbounds": [
            {
                "protocol": "failover",
                "settings": {
                    "actors": [
                        "ss",
                        "direct"
                    ]
                },
                "tag": "failover_out"
            },
            {
                "protocol": "shadowsocks",
                "settings": {
                    "address": "45.88.42.58",
                    "method": "aes-256-gcm",
                    "password": "vS4y7Vm0a213il9o",
                    "port": 37415
                },
                "tag":"ss"
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
                    "domainKeyword": [
                        "ipinfo"
                    ],
                    "target": "direct"
                }
            ]
        }
        
    }
    "#;

    // let base_dir = env::current_dir().expect("not found path");
    // let configPath = String::from(base_dir.to_str().expect("msg")) + "/leaf-client/src/config.json";

    // if let Err(e) = leaf::test_config(&configPath) {
    //     panic!("{}",e);
    // }
    let opts = leaf::StartOptions {
        config: leaf::Config::Str(config.to_string()),
        auto_reload: false,
        runtime_opt: leaf::RuntimeOption::SingleThread,
    };
    if let Err(e) = leaf::start(0, opts) {
        panic!("{}",e);
    }


    print!("end")

}