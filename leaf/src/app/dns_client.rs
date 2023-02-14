use std::collections::HashMap;
use std::error::Error;
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use futures::future::select_ok;
use log::*;
use lru::LruCache;
use rand::{rngs::StdRng, Rng, SeedableRng};
use reqwest::Url;
use serde_json::Value;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::timeout;
use trust_dns_proto::{
    op::{
        header::MessageType, op_code::OpCode, query::Query, response_code::ResponseCode, Message,
    },
    rr::{record_data::RData, record_type::RecordType, Name},
};

use crate::{app::dispatcher::Dispatcher, option, proxy::*, session::*};

#[derive(Clone, Debug)]
struct CacheEntry {
    pub ips: Vec<IpAddr>,
    // The deadline this entry should be considered expired.
    pub deadline: Instant,
}

pub struct DnsClient {
    dispatcher: Option<Weak<Dispatcher>>,
    servers: Vec<SocketAddr>,
    hosts: HashMap<String, Vec<IpAddr>>,
    ipv4_cache: Arc<TokioMutex<LruCache<String, CacheEntry>>>,
    ipv6_cache: Arc<TokioMutex<LruCache<String, CacheEntry>>>,
    last_doh_timeout_time:Arc<TokioMutex<Vec<Instant>>>,
    doh_keys: Arc<HashMap<String,String>>,
}

impl DnsClient {
    fn load_servers(dns: &crate::config::Dns) -> Result<Vec<SocketAddr>> {
        let mut servers = Vec::new();
        for server in dns.servers.iter() {
            servers.push(SocketAddr::new(server.parse::<IpAddr>()?, 53));
        }
        if servers.is_empty() {
            return Err(anyhow!("no dns servers"));
        }
        Ok(servers)
    }

    fn load_hosts(dns: &crate::config::Dns) -> HashMap<String, Vec<IpAddr>> {
        let mut hosts = HashMap::new();
        for (name, ips) in dns.hosts.iter() {
            hosts.insert(name.to_owned(), ips.values.to_vec());
        }
        let mut parsed_hosts = HashMap::new();
        for (name, static_ips) in hosts.iter() {
            let mut ips = Vec::new();
            for ip in static_ips {
                if let Ok(parsed_ip) = ip.parse::<IpAddr>() {
                    ips.push(parsed_ip);
                }
            }
            parsed_hosts.insert(name.to_owned(), ips);
        }
        parsed_hosts
    }

    fn load_dohkeys(dns: &crate::config::Dns) -> HashMap<String,String> {
        let mut keys = HashMap::new();
        for (k,v) in &dns.doh_keys {
            keys.insert(k.to_string(), v.to_string());
        }
        keys
    }

    pub fn new(dns: &protobuf::MessageField<crate::config::Dns>) -> Result<Self> {
        let dns = if let Some(dns) = dns.as_ref() {
            dns
        } else {
            return Err(anyhow!("empty dns config"));
        };
        let servers = Self::load_servers(dns)?;
        let hosts = Self::load_hosts(dns);
        let dohkeys = Self::load_dohkeys(dns);
        let ipv4_cache = Arc::new(TokioMutex::new(LruCache::<String, CacheEntry>::new(
            *option::DNS_CACHE_SIZE,
        )));
        let ipv6_cache = Arc::new(TokioMutex::new(LruCache::<String, CacheEntry>::new(
            *option::DNS_CACHE_SIZE,
        )));


        Ok(Self {
            dispatcher: None,
            servers,
            hosts,
            ipv4_cache,
            ipv6_cache,
            last_doh_timeout_time:Arc::new(TokioMutex::new(Vec::new())),
            doh_keys:Arc::new(dohkeys),
        })
    }

    pub fn replace_dispatcher(&mut self, dispatcher: Weak<Dispatcher>) {
        self.dispatcher.replace(dispatcher);
    }

    pub fn reload(&mut self, dns: &protobuf::MessageField<crate::config::Dns>) -> Result<()> {
        let dns = if let Some(dns) = dns.as_ref() {
            dns
        } else {
            return Err(anyhow!("empty dns config"));
        };
        let servers = Self::load_servers(dns)?;
        let hosts = Self::load_hosts(dns);
        self.servers = servers;
        self.hosts = hosts;
        Ok(())
    }

    async fn optimize_cache_ipv4(&self, address: String, connected_ip: IpAddr) {
        // Nothing to do if the target address is an IP address.
        if address.parse::<IpAddr>().is_ok() {
            return;
        }

        // If the connected IP is not in the first place, we should optimize it.
        let mut new_entry = if let Some(entry) = self.ipv4_cache.lock().await.get(&address) {
            if !entry.ips.starts_with(&[connected_ip]) && entry.ips.contains(&connected_ip) {
                entry.clone()
            } else {
                return;
            }
        } else {
            return;
        };

        // Move failed IPs to the end, the optimized vector starts with the connected IP.
        if let Ok(idx) = new_entry.ips.binary_search(&connected_ip) {
            trace!("updates DNS cache item from\n{:#?}", &new_entry);
            new_entry.ips.rotate_left(idx);
            trace!("to\n{:#?}", &new_entry);
            self.ipv4_cache.lock().await.put(address, new_entry);
            trace!("updated cache");
        }
    }

    async fn optimize_cache_ipv6(&self, address: String, connected_ip: IpAddr) {
        // Nothing to do if the target address is an IP address.
        if address.parse::<IpAddr>().is_ok() {
            return;
        }

        // If the connected IP is not in the first place, we should optimize it.
        let mut new_entry = if let Some(entry) = self.ipv6_cache.lock().await.get(&address) {
            if !entry.ips.starts_with(&[connected_ip]) && entry.ips.contains(&connected_ip) {
                entry.clone()
            } else {
                return;
            }
        } else {
            return;
        };

        // Move failed IPs to the end, the optimized vector starts with the connected IP.
        if let Ok(idx) = new_entry.ips.binary_search(&connected_ip) {
            trace!("updates DNS cache item from\n{:#?}", &new_entry);
            new_entry.ips.rotate_left(idx);
            trace!("to\n{:#?}", &new_entry);
            self.ipv6_cache.lock().await.put(address, new_entry);
            trace!("updated cache");
        }
    }

    /// Updates the cache according to the IP address successfully connected.
    pub async fn optimize_cache(&self, address: String, connected_ip: IpAddr) {
        match connected_ip {
            IpAddr::V4(..) => self.optimize_cache_ipv4(address, connected_ip).await,
            IpAddr::V6(..) => self.optimize_cache_ipv6(address, connected_ip).await,
        }
    }

    async fn query_task(
        &self,
        is_direct: bool,
        request: Vec<u8>,
        host: &str,
        server: &SocketAddr,
    ) -> Result<CacheEntry> {

        if is_direct{
            let r = self.startdoh(&host.to_string()).await;
            if r.is_ok(){
                return Ok(r.unwrap());
            }
        }
        let socket = if is_direct {
            let socket = self.new_udp_socket(server).await?;
            Box::new(StdOutboundDatagram::new(socket))
        } else {
            if let Some(dispatcher_weak) = self.dispatcher.as_ref() {
                let sess = Session {
                    network: Network::Udp,
                    destination: SocksAddr::from(server),
                    ..Default::default()
                };
                if let Some(dispatcher) = dispatcher_weak.upgrade() {
                    dispatcher.dispatch_datagram(sess).await?
                } else {
                    return Err(anyhow!("dispatcher is deallocated"));
                }
            } else {
                return Err(anyhow!("could not find a dispatcher"));
            }
        };
        let (mut r, mut s) = socket.split();
        let server = SocksAddr::from(server);
        let mut last_err = None;
        for _i in 0..*option::MAX_DNS_RETRIES {
            debug!("looking up host {} on {},direct:{}", host, server,is_direct);
            let start = tokio::time::Instant::now();
            match s.send_to(&request, &server).await {
                Ok(_) => {
                    let mut buf = vec![0u8; 512];
                    match timeout(
                        Duration::from_secs(*option::DNS_TIMEOUT),
                        r.recv_from(&mut buf),
                    )
                    .await
                    {
                        Ok(res) => match res {
                            Ok((n, _)) => {
                                let resp = match Message::from_vec(&buf[..n]) {
                                    Ok(resp) => resp,
                                    Err(err) => {
                                        last_err = Some(anyhow!("parse message failed: {:?}", err));
                                        // broken response, no retry
                                        break;
                                    }
                                };
                                if resp.response_code() != ResponseCode::NoError {
                                    last_err =
                                        Some(anyhow!("response error {}", resp.response_code()));
                                    // error response, no retry
                                    //
                                    // TODO Needs more careful investigations, I'm not quite sure about
                                    // this.
                                    break;
                                }
                                let mut ips = Vec::new();
                                for ans in resp.answers() {
                                    // TODO checks?
                                    match ans.rdata() {
                                        RData::A(ip) => {
                                            ips.push(IpAddr::V4(ip.to_owned()));
                                        }
                                        RData::AAAA(ip) => {
                                            ips.push(IpAddr::V6(ip.to_owned()));
                                        }
                                        _ => (),
                                    }
                                }
                                if !ips.is_empty() {
                                    let elapsed = tokio::time::Instant::now().duration_since(start);
                                    let ttl = resp.answers().iter().next().unwrap().ttl();
                                    debug!(
                                        "looking up return {} ips (ttl {}) for {} from {} in {}ms : {:?}",
                                        ips.len(),
                                        ttl,
                                        host,
                                        server,
                                        elapsed.as_millis(),ips,
                                    );
                                    let deadline = if let Some(d) =
                                        Instant::now().checked_add(Duration::from_secs(ttl.into()))
                                    {
                                        d
                                    } else {
                                        last_err = Some(anyhow!("invalid ttl"));
                                        break;
                                    };
                                    let entry = CacheEntry { ips, deadline };
                                    trace!("ips for {}:\n{:#?}", host, &entry);
                                    return Ok(entry);
                                } else {
                                    // response with 0 records
                                    //
                                    // TODO Not sure how to due with this.
                                    last_err = Some(anyhow!("no records"));
                                    break;
                                }
                            }
                            Err(err) => {
                                last_err = Some(anyhow!("recv failed: {:?}", err));
                                // socket recv_from error, retry
                            }
                        },
                        Err(e) => {
                            last_err = Some(anyhow!("recv timeout: {}", e));
                            // timeout, retry
                        }
                    }
                }
                Err(err) => {
                    last_err = Some(anyhow!("send failed: {:?}", err));
                    // socket send_to error, retry
                }
            }
        }
        Err(last_err.unwrap_or_else(|| anyhow!("all lookup attempts failed")))
    }

    fn new_query(name: Name, ty: RecordType) -> Message {
        let mut msg = Message::new();
        msg.add_query(Query::query(name, ty));
        let mut rng = StdRng::from_entropy();
        let id: u16 = rng.gen();
        msg.set_id(id);
        msg.set_op_code(OpCode::Query);
        msg.set_message_type(MessageType::Query);
        msg.set_recursion_desired(true);
        msg
    }

    async fn cache_insert(&self, host: &str, entry: CacheEntry) {
        if entry.ips.is_empty() {
            return;
        }
        match entry.ips[0] {
            IpAddr::V4(..) => self.ipv4_cache.lock().await.put(host.to_owned(), entry),
            IpAddr::V6(..) => self.ipv6_cache.lock().await.put(host.to_owned(), entry),
        };
    }

    async fn get_cached(&self, host: &String) -> Result<Vec<IpAddr>> {
        let mut cached_ips = Vec::new();

        // TODO reduce boilerplates
        match (*crate::option::ENABLE_IPV6, *crate::option::PREFER_IPV6) {
            (true, true) => {
                if let Some(entry) = self.ipv6_cache.lock().await.get(host) {
                    if entry
                        .deadline
                        .checked_duration_since(Instant::now())
                        .is_none()
                    {
                        return Err(anyhow!("entry expired"));
                    }
                    let mut ips = entry.ips.to_vec();
                    cached_ips.append(&mut ips);
                }
                if let Some(entry) = self.ipv4_cache.lock().await.get(host) {
                    if entry
                        .deadline
                        .checked_duration_since(Instant::now())
                        .is_none()
                    {
                        return Err(anyhow!("entry expired"));
                    }
                    let mut ips = entry.ips.to_vec();
                    cached_ips.append(&mut ips);
                }
            }
            (true, false) => {
                if let Some(entry) = self.ipv4_cache.lock().await.get(host) {
                    if entry
                        .deadline
                        .checked_duration_since(Instant::now())
                        .is_none()
                    {
                        return Err(anyhow!("entry expired"));
                    }
                    let mut ips = entry.ips.to_vec();
                    cached_ips.append(&mut ips);
                }
                if let Some(entry) = self.ipv6_cache.lock().await.get(host) {
                    if entry
                        .deadline
                        .checked_duration_since(Instant::now())
                        .is_none()
                    {
                        return Err(anyhow!("entry expired"));
                    }
                    let mut ips = entry.ips.to_vec();
                    cached_ips.append(&mut ips);
                }
            }
            _ => {
                if let Some(entry) = self.ipv4_cache.lock().await.get(host) {
                    if entry
                        .deadline
                        .checked_duration_since(Instant::now())
                        .is_none()
                    {
                        return Err(anyhow!("entry expired"));
                    }
                    let mut ips = entry.ips.to_vec();
                    cached_ips.append(&mut ips);
                }
            }
        }

        if !cached_ips.is_empty() {
            Ok(cached_ips)
        } else {
            Err(anyhow!("empty result"))
        }
    }

    pub async fn lookup(&self, host: &String) -> Result<Vec<IpAddr>> {
        self._lookup(host, false).await
    }

    pub async fn direct_lookup(&self, host: &String) -> Result<Vec<IpAddr>> {
        self._lookup(host, true).await
    }

    pub async fn _lookup(&self, host: &String, is_direct: bool) -> Result<Vec<IpAddr>> {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }

        if let Ok(ips) = self.get_cached(host).await {
            return Ok(ips);
        }

        // Making cache lookup a priority rather than static hosts lookup
        // and insert the static IPs to the cache because there's a chance
        // for the IPs in the cache to be re-ordered.
        if !self.hosts.is_empty() {
            if let Some(ips) = self.hosts.get(host) {
                if !ips.is_empty() {
                    if ips.len() > 1 {
                        let deadline = Instant::now()
                            .checked_add(Duration::from_secs(6000))
                            .unwrap();
                        self.cache_insert(
                            host,
                            CacheEntry {
                                ips: ips.clone(),
                                deadline,
                            },
                        )
                        .await;
                    }
                    return Ok(ips.to_vec());
                }
            }
        }

        let mut fqdn = host.to_owned();
        fqdn.push('.');
        let name = match Name::from_str(&fqdn) {
            Ok(n) => n,
            Err(e) => return Err(anyhow!("invalid domain name [{}]: {}", host, e)),
        };

        let mut query_tasks = Vec::new();

        // TODO reduce boilerplates
        match (*crate::option::ENABLE_IPV6, *crate::option::PREFER_IPV6) {
            (true, true) => {
                let msg = Self::new_query(name.clone(), RecordType::AAAA);
                let msg_buf = match msg.to_vec() {
                    Ok(b) => b,
                    Err(e) => return Err(anyhow!("encode message to buffer failed: {}", e)),
                };
                let mut tasks = Vec::new();
                for server in &self.servers {
                    let t = self.query_task(is_direct, msg_buf.clone(), host, server);
                    tasks.push(Box::pin(t));
                }
                let query_task = select_ok(tasks.into_iter());
                query_tasks.push(query_task);

                let msg = Self::new_query(name.clone(), RecordType::A);
                let msg_buf = match msg.to_vec() {
                    Ok(b) => b,
                    Err(e) => return Err(anyhow!("encode message to buffer failed: {}", e)),
                };
                let mut tasks = Vec::new();
                for server in &self.servers {
                    let t = self.query_task(is_direct, msg_buf.clone(), host, server);
                    tasks.push(Box::pin(t));
                }
                let query_task = select_ok(tasks.into_iter());
                query_tasks.push(query_task);
            }
            (true, false) => {
                let msg = Self::new_query(name.clone(), RecordType::A);
                let msg_buf = match msg.to_vec() {
                    Ok(b) => b,
                    Err(e) => return Err(anyhow!("encode message to buffer failed: {}", e)),
                };
                let mut tasks = Vec::new();
                for server in &self.servers {
                    let t = self.query_task(is_direct, msg_buf.clone(), host, server);
                    tasks.push(Box::pin(t));
                }
                let query_task = select_ok(tasks.into_iter());
                query_tasks.push(query_task);

                let msg = Self::new_query(name.clone(), RecordType::AAAA);
                let msg_buf = match msg.to_vec() {
                    Ok(b) => b,
                    Err(e) => return Err(anyhow!("encode message to buffer failed: {}", e)),
                };
                let mut tasks = Vec::new();
                for server in &self.servers {
                    let t = self.query_task(is_direct, msg_buf.clone(), host, server);
                    tasks.push(Box::pin(t));
                }
                let query_task = select_ok(tasks.into_iter());
                query_tasks.push(query_task);
            }
            _ => {
                let msg = Self::new_query(name.clone(), RecordType::A);
                let msg_buf = match msg.to_vec() {
                    Ok(b) => b,
                    Err(e) => return Err(anyhow!("encode message to buffer failed: {}", e)),
                };
                let mut tasks = Vec::new();
                for server in &self.servers {
                    let t = self.query_task(is_direct, msg_buf.clone(), host, server);
                    tasks.push(Box::pin(t));
                }
                let query_task = select_ok(tasks.into_iter());
                query_tasks.push(query_task);
            }
        }

        let mut ips = Vec::new();
        let mut last_err = None;

        for v in futures::future::join_all(query_tasks).await {
            match v {
                Ok(mut v) => {
                    self.cache_insert(host, v.0.clone()).await;
                    ips.append(&mut v.0.ips);
                }
                Err(e) => last_err = Some(anyhow!("all dns servers failed, last error: {}", e)),
            }
        }

        if !ips.is_empty() {
            return Ok(ips);
        }

        Err(last_err.unwrap_or_else(|| anyhow!("could not resolve to any address")))
    }

    async fn startdoh(&self,host:&String) -> Result<CacheEntry,()> {
        let start = Instant::now();
        let mut last = self.last_doh_timeout_time.lock().await;
        if last.len() > 0{
            let interval = start.duration_since(*last.last().unwrap());
            let sec = interval.as_secs();
            if sec < 3{
                return Err(());
            }
            last.clear();
        }
        let ips = doh(host.to_string(),self.doh_keys.to_owned()).await;
        let interval = Instant::now().checked_duration_since(start).unwrap();
        match ips {
            Ok(v) =>{
                if v.len() > 0{
                    let ips : Vec<IpAddr> = v.iter().map(|v|{
                        IpAddr::from_str(&v.0).ok()
                    }).filter(|v|{v.is_some()}).map(|v|{v.unwrap()}).collect();
                    let ttl = v.last().unwrap().1;
                    let deadline = Instant::now().checked_add(Duration::from_secs(ttl)).unwrap();
                    let entry = CacheEntry { 
                        ips,  deadline};
                    debug!("looking dns {} from doh: result:{:?} time {:?}",host,v,interval);
                    return Ok(entry);
                }
            }
            Err(_) => {
                last.push(Instant::now());
            }
        }

        return Err(());
    }
}

impl UdpConnector for DnsClient {}


async fn cloud_fare_doh(host:String) -> Result<Vec<(String,u64)>, Box<dyn Error>>{
        let url = Url::parse(format!("https://1.1.1.1/dns-query?name={}",host).as_str())?;
        
        let client = reqwest::Client::builder().timeout(Duration::from_secs(2)).build()?;

        let response = client.get(url)
        .header("accept", "application/dns-json")
        .send().await?;

        let result = response.json::<HashMap<String, Value>>().await?;
        
        let answers = result.get("Answer").map(|v|{
            v.as_array()
        });

        if let Some(Some(answers)) = answers {
            let v:Vec<(String,u64)> = answers.iter().map(|v|{
                if let Some((Some(Some(v)),Some(Some(t)))) = v.as_object().map(|v|{
                    (v.get("data").map(|v|{v.as_str()}),
                    v.get("TTL").map(|v|{v.as_u64()}))
                }) {
                    return Some((v.to_string(),t));
                }
                return None;
            }).filter(|v|{v.is_some()}).map(|v|{v.unwrap()}).collect();

            return Ok(v);
        }

    let v : Vec<(String,u64)> = Vec::new();
    return Ok(v)
}

async fn doh(host:String,keys:Arc<HashMap<String,String>>) -> Result<Vec<(String,u64)>, ()>{
    let mut v : Vec<(String,u64)> = Vec::new();
    for (key,value) in keys.iter(){
        if key == "1.1.1.1"{
            match cloud_fare_doh(host.to_owned()).await{
                Ok( r) =>{
                    v.append(&mut r.to_vec());
                }
                Err(_) =>{}
            }
        }
    }

    return Ok(v);
}