use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use async_trait::async_trait;
use futures_util::future::select_ok;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use hickory_proto::op::Message;
use crate::pipeline::context::QueryContext;
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

const STREAM_POOL_MAX_IDLE: usize = 4;
const STREAM_POOL_IDLE_TTL: Duration = Duration::from_secs(45);

fn shared_tls_connector() -> &'static tokio_rustls::TlsConnector {
    static TLS_CONNECTOR: OnceLock<tokio_rustls::TlsConnector> = OnceLock::new();
    TLS_CONNECTOR.get_or_init(|| {
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        let client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        tokio_rustls::TlsConnector::from(Arc::new(client_config))
    })
}

#[derive(Clone, Default)]
pub struct TcpConnPool(Arc<parking_lot::Mutex<Vec<(TcpStream, Instant)>>>);

impl std::fmt::Debug for TcpConnPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TcpConnPool(idle={})", self.0.lock().len())
    }
}

impl TcpConnPool {
    fn pop_fresh(&self) -> Option<TcpStream> {
        let mut guard = self.0.lock();
        while let Some((stream, ts)) = guard.pop() {
            if ts.elapsed() < STREAM_POOL_IDLE_TTL {
                return Some(stream);
            }
        }
        None
    }

    fn put(&self, stream: TcpStream) {
        let mut guard = self.0.lock();
        guard.retain(|(_, ts)| ts.elapsed() < STREAM_POOL_IDLE_TTL);
        if guard.len() < STREAM_POOL_MAX_IDLE {
            guard.push((stream, Instant::now()));
        }
    }

    pub fn clear(&self) {
        self.0.lock().clear();
    }

    #[allow(dead_code)]
    pub fn idle_len(&self) -> usize {
        self.0.lock().len()
    }
}

#[derive(Clone, Default)]
pub struct DotConnPool(Arc<parking_lot::Mutex<Vec<(tokio_rustls::client::TlsStream<TcpStream>, Instant)>>>);

impl std::fmt::Debug for DotConnPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "DotConnPool(idle={})", self.0.lock().len())
    }
}

impl DotConnPool {
    fn pop_fresh(&self) -> Option<tokio_rustls::client::TlsStream<TcpStream>> {
        let mut guard = self.0.lock();
        while let Some((stream, ts)) = guard.pop() {
            if ts.elapsed() < STREAM_POOL_IDLE_TTL {
                return Some(stream);
            }
        }
        None
    }

    fn put(&self, stream: tokio_rustls::client::TlsStream<TcpStream>) {
        let mut guard = self.0.lock();
        guard.retain(|(_, ts)| ts.elapsed() < STREAM_POOL_IDLE_TTL);
        if guard.len() < STREAM_POOL_MAX_IDLE {
            guard.push((stream, Instant::now()));
        }
    }

    pub fn clear(&self) {
        self.0.lock().clear();
    }

    #[allow(dead_code)]
    pub fn idle_len(&self) -> usize {
        self.0.lock().len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UpstreamProtocol {
    Udp,
    Tcp,
    Dot,
    Doh,
}

fn default_connect_ip_version() -> String {
    "dual".to_string()
}

fn default_cooldown() -> Arc<parking_lot::RwLock<Instant>> {
    Arc::new(parking_lot::RwLock::new(Instant::now()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpstreamServer {
    pub tag: String,
    pub addr: String,
    pub protocol: UpstreamProtocol,
    pub bootstrap_ip: Option<String>,
    pub host_name: Option<String>,
    pub proxy: Option<String>,
    #[serde(default = "default_connect_ip_version")]
    pub connect_ip_version: String,
    #[serde(skip)]
    pub doh_client: Arc<parking_lot::RwLock<Option<reqwest::Client>>>,
    #[serde(skip, default)]
    pub tcp_pool: TcpConnPool,
    #[serde(skip, default)]
    pub dot_pool: DotConnPool,
    #[serde(skip, default)]
    pub fail_count: Arc<AtomicU32>,
    #[serde(skip, default)]
    pub ewma_rtt_us: Arc<AtomicU64>,
    #[serde(skip, default = "default_cooldown")]
    pub cooldown_until: Arc<parking_lot::RwLock<Instant>>,
}

fn normalize_ip_ver(ver: &str) -> String {
    match ver.trim().to_ascii_lowercase().as_str() {
        "ipv4" | "ipv4_only" => "ipv4".to_string(),
        "ipv6" | "ipv6_only" => "ipv6".to_string(),
        _ => "dual".to_string(),
    }
}

fn check_ip_version_allowed(addr: &SocketAddr, connect_ip_version: &str) -> Result<(), String> {
    match connect_ip_version {
        "ipv4" if addr.is_ipv6() => Err("上游目标为 IPv6，已被 connect_ip_version=ipv4 策略跳过".to_string()),
        "ipv6" if addr.is_ipv4() => Err("上游目标为 IPv4，已被 connect_ip_version=ipv6 策略跳过".to_string()),
        _ => Ok(()),
    }
}

async fn resolve_upstream_socket_addr(
    raw_addr: &str,
    default_port: u16,
    connect_ip_version: &str,
) -> Result<(SocketAddr, Option<String>), String> {
    let with_port = if raw_addr.starts_with('[') {
        if raw_addr.contains("]:") {
            raw_addr.to_string()
        } else {
            format!("{}:{}", raw_addr, default_port)
        }
    } else if raw_addr.contains(':') {
        // Distinguish IPv6 without port vs host:port
        if IpAddr::from_str(raw_addr).is_ok() {
            format!("[{}]:{}", raw_addr, default_port)
        } else {
            raw_addr.to_string()
        }
    } else {
        format!("{}:{}", raw_addr, default_port)
    };

    if let Ok(sa) = with_port.parse::<SocketAddr>() {
        return Ok((sa, None));
    }

    let inferred_host = with_port
        .rsplit_once(':')
        .map(|(h, _)| h.trim_matches(|c| c == '[' || c == ']').to_string());

    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(&with_port)
        .await
        .map_err(|e| format!("无法解析上游主机 {}: {}", with_port, e))?
        .collect();

    let picked = addrs
        .iter()
        .copied()
        .find(|sa| check_ip_version_allowed(sa, connect_ip_version).is_ok())
        .or_else(|| addrs.first().copied())
        .ok_or_else(|| format!("上游主机无可用解析地址: {}", with_port))?;

    Ok((picked, inferred_host))
}

async fn exchange_framed_dns<S>(
    stream: &mut S,
    framed_req: &[u8],
    timeout_duration: Duration,
) -> Result<Message, (bool, String)>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if let Err(e) = timeout(timeout_duration, stream.write_all(framed_req)).await {
        return Err((true, format!("Write timeout: {}", e)));
    } else if let Err(e) = stream.flush().await {
        return Err((true, format!("Flush error: {}", e)));
    }

    let mut resp_len_buf = [0u8; 2];
    match timeout(timeout_duration, stream.read_exact(&mut resp_len_buf)).await {
        Err(_) => return Err((false, "DNS stream read len timeout".to_string())),
        Ok(Err(e)) => return Err((true, e.to_string())),
        Ok(Ok(_)) => {}
    }

    let resp_len = u16::from_be_bytes(resp_len_buf) as usize;
    if resp_len == 0 || resp_len > 65535 {
        return Err((false, "Invalid DNS TCP/DoT response length".to_string()));
    }

    let mut resp_buf = vec![0u8; resp_len];
    timeout(timeout_duration, stream.read_exact(&mut resp_buf))
        .await
        .map_err(|_| (false, "DNS stream read body timeout".to_string()))?
        .map_err(|e| (false, e.to_string()))?;

    Message::from_vec(&resp_buf).map_err(|e| (false, e.to_string()))
}

impl UpstreamServer {
    pub fn set_proxy(&mut self, proxy: Option<String>) {
        self.proxy = proxy;
        *self.doh_client.write() = None;
        self.tcp_pool.clear();
        self.dot_pool.clear();
        self.fail_count.store(0, Ordering::Relaxed);
        self.ewma_rtt_us.store(0, Ordering::Relaxed);
        *self.cooldown_until.write() = Instant::now();
    }

    pub fn set_connect_ip_version(&mut self, ver: &str) {
        let norm = normalize_ip_ver(ver);
        if self.connect_ip_version != norm {
            self.connect_ip_version = norm;
            *self.doh_client.write() = None;
            self.tcp_pool.clear();
            self.dot_pool.clear();
        }
    }

    #[allow(dead_code)]
    pub fn ewma_rtt_ms(&self) -> Option<f64> {
        let us = self.ewma_rtt_us.load(Ordering::Relaxed);
        if us == 0 {
            None
        } else {
            Some((us as f64) / 1000.0)
        }
    }

    pub fn get_or_init_doh_client(&self, timeout_duration: Duration) -> Result<reqwest::Client, String> {
        if let Some(ref c) = *self.doh_client.read() {
            return Ok(c.clone());
        }

        let mut w = self.doh_client.write();
        if let Some(ref c) = *w {
            return Ok(c.clone());
        }

        let mut builder = reqwest::Client::builder()
            .timeout(timeout_duration)
            .pool_idle_timeout(Some(Duration::from_secs(90)))
            .pool_max_idle_per_host(8)
            .tcp_keepalive(Some(Duration::from_secs(60)))
            .http2_keep_alive_interval(Some(Duration::from_secs(30)))
            .http2_keep_alive_while_idle(true)
            .http2_keep_alive_timeout(Duration::from_secs(10))
            .http2_adaptive_window(true);

        let mut has_proxy = false;
        if let Some(ref p) = self.proxy {
            if p != "none" && !p.is_empty() {
                has_proxy = true;
                let mut p_addr = p.clone();
                if !p_addr.contains("://") {
                    p_addr = format!("socks5h://{}", p_addr);
                } else if p_addr.starts_with("socks5://") {
                    p_addr = p_addr.replacen("socks5://", "socks5h://", 1);
                }
                match reqwest::Proxy::all(&p_addr) {
                    Ok(proxy) => {
                        builder = builder.proxy(proxy);
                    }
                    Err(e) => {
                        return Err(format!("代理配置无效 ({}): {}", p_addr, e));
                    }
                }
            }
        }

        if !has_proxy {
            match self.connect_ip_version.as_str() {
                "ipv4" => {
                    builder = builder.local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
                }
                "ipv6" => {
                    builder = builder.local_address(IpAddr::V6(Ipv6Addr::UNSPECIFIED));
                }
                _ => {}
            }
        }

        // Handle -host-name / #sni= override (e.g. https://101.226.4.6/dns-query#sni=doh.360.cn)
        if let Some(ref host) = self.host_name {
            if let Some(rest) = self.addr.strip_prefix("https://") {
                let (host_port_str, _path) = match rest.find('/') {
                    Some(idx) => (&rest[..idx], &rest[idx..]),
                    None => (rest, "/dns-query"),
                };
                let parsed_sa: Option<SocketAddr> = if host_port_str.starts_with('[') {
                    if let Some((bracket_ip, port_str)) = host_port_str.rsplit_once("]:") {
                        let clean_ip = bracket_ip.trim_start_matches('[');
                        IpAddr::from_str(clean_ip)
                            .ok()
                            .map(|ip| SocketAddr::new(ip, port_str.parse().unwrap_or(443)))
                    } else {
                        let clean_ip = host_port_str.trim_matches(|c| c == '[' || c == ']');
                        IpAddr::from_str(clean_ip).ok().map(|ip| SocketAddr::new(ip, 443))
                    }
                } else {
                    let (ip_str, port) = match host_port_str.split_once(':') {
                        Some((ip_part, port_part)) => (ip_part, port_part.parse().unwrap_or(443)),
                        None => (host_port_str, 443),
                    };
                    IpAddr::from_str(ip_str).ok().map(|ip| SocketAddr::new(ip, port))
                };

                if let Some(sa) = parsed_sa {
                    if !has_proxy {
                        check_ip_version_allowed(&sa, &self.connect_ip_version)?;
                    }
                    builder = builder.resolve(host, sa);
                }
            }
        }

        let client = builder.build().map_err(|e| e.to_string())?;
        *w = Some(client.clone());
        Ok(client)
    }

    pub fn parse(tag: &str, raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut parts = trimmed.split_whitespace();
        let mut uri = String::new();
        let mut host_name: Option<String> = None;
        let mut proxy: Option<String> = None;

        while let Some(part) = parts.next() {
            if part == "server-https" || part == "server-tcp" || part == "server-udp" {
                continue;
            } else if part == "-host-name" || part == "-sni" {
                if let Some(h) = parts.next() {
                    host_name = Some(h.to_string());
                }
            } else if part == "-proxy" {
                if let Some(p) = parts.next() {
                    proxy = Some(p.to_string());
                }
            } else if part.starts_with('-') {
                let _ = parts.next();
            } else if uri.is_empty() {
                uri = part.to_string();
            }
        }

        if uri.is_empty() {
            return None;
        }

        // Support NexDNS native URI fragment metadata:
        // e.g. https://101.226.4.6/dns-query#sni=doh.360.cn&proxy=default
        if let Some(pos) = uri.find('#') {
            let frag = uri[pos + 1..].to_string();
            uri.truncate(pos);

            if frag.contains('=') {
                for pair in frag.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        match k.trim() {
                            "sni" | "host" => {
                                if host_name.is_none() {
                                    host_name = Some(v.trim().to_string());
                                }
                            }
                            "proxy" => {
                                if proxy.is_none() {
                                    proxy = Some(v.trim().to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                }
            } else if !frag.trim().is_empty() && host_name.is_none() {
                host_name = Some(frag.trim().to_string());
            }
        }

        let (protocol, clean_addr) = if let Some(rest) = uri.strip_prefix("https://") {
            (UpstreamProtocol::Doh, format!("https://{}", rest))
        } else if let Some(rest) = uri.strip_prefix("tls://") {
            (UpstreamProtocol::Dot, rest.to_string())
        } else if let Some(rest) = uri.strip_prefix("tcp://") {
            (UpstreamProtocol::Tcp, rest.to_string())
        } else if let Some(rest) = uri.strip_prefix("udp://") {
            (UpstreamProtocol::Udp, rest.to_string())
        } else {
            (UpstreamProtocol::Udp, uri)
        };

        Some(Self {
            tag: tag.to_string(),
            addr: clean_addr,
            protocol,
            bootstrap_ip: None,
            host_name,
            proxy,
            connect_ip_version: default_connect_ip_version(),
            doh_client: Arc::new(parking_lot::RwLock::new(None)),
            tcp_pool: TcpConnPool::default(),
            dot_pool: DotConnPool::default(),
            fail_count: Arc::new(AtomicU32::new(0)),
            ewma_rtt_us: Arc::new(AtomicU64::new(0)),
            cooldown_until: Arc::new(parking_lot::RwLock::new(Instant::now())),
        })
    }

    pub async fn query(&self, req_bytes: &[u8], timeout_duration: Duration) -> Result<Message, String> {
        let now = Instant::now();
        if now < *self.cooldown_until.read() {
            return Err(format!("上游节点 [{}] 熔断冷却中", self.addr));
        }

        let start = Instant::now();
        let res = self.query_internal(req_bytes, timeout_duration).await;
        match &res {
            Ok(_) => {
                let rtt_us = (start.elapsed().as_micros() as u64).max(1);
                let prev = self.ewma_rtt_us.load(Ordering::Relaxed);
                let next = if prev == 0 {
                    rtt_us
                } else {
                    (prev * 7 + rtt_us * 3) / 10
                };
                self.ewma_rtt_us.store(next, Ordering::Relaxed);
                self.fail_count.store(0, Ordering::Relaxed);
            }
            Err(_) => {
                let penalty_us = (timeout_duration.as_micros() as u64).max(500_000);
                let prev = self.ewma_rtt_us.load(Ordering::Relaxed);
                let next = if prev == 0 {
                    penalty_us
                } else {
                    (prev.saturating_mul(2)).max(penalty_us).min(10_000_000)
                };
                self.ewma_rtt_us.store(next, Ordering::Relaxed);

                let cnt = self.fail_count.fetch_add(1, Ordering::Relaxed) + 1;
                if cnt >= 2 {
                    let cooldown_secs = match cnt {
                        2..=3 => 10,
                        4..=7 => 30,
                        _ => 60,
                    };
                    let mut w = self.cooldown_until.write();
                    *w = Instant::now() + Duration::from_secs(cooldown_secs);
                }
            }
        }
        res
    }

    async fn query_internal(&self, req_bytes: &[u8], timeout_duration: Duration) -> Result<Message, String> {
        let has_proxy = self.proxy.as_deref().map(|p| p != "none" && !p.is_empty()).unwrap_or(false);

        match self.protocol {
            UpstreamProtocol::Udp => {
                let (socket_addr, _) =
                    resolve_upstream_socket_addr(&self.addr, 53, &self.connect_ip_version).await?;
                check_ip_version_allowed(&socket_addr, &self.connect_ip_version)?;

                let local_bind = if socket_addr.is_ipv6() { "[::]:0" } else { "0.0.0.0:0" };
                let socket = UdpSocket::bind(local_bind).await.map_err(|e| e.to_string())?;
                socket.connect(socket_addr).await.map_err(|e| e.to_string())?;
                socket.send(req_bytes).await.map_err(|e| e.to_string())?;

                let mut buf = vec![0u8; 4096];
                let len = timeout(timeout_duration, socket.recv(&mut buf))
                    .await
                    .map_err(|_| "UDP query timeout".to_string())?
                    .map_err(|e| e.to_string())?;

                Message::from_vec(&buf[..len]).map_err(|e| e.to_string())
            }
            UpstreamProtocol::Tcp => {
                let mut framed = Vec::with_capacity(2 + req_bytes.len());
                framed.extend_from_slice(&(req_bytes.len() as u16).to_be_bytes());
                framed.extend_from_slice(req_bytes);

                if let Some(mut pooled) = self.tcp_pool.pop_fresh() {
                    match exchange_framed_dns(&mut pooled, &framed, timeout_duration).await {
                        Ok(msg) => {
                            self.tcp_pool.put(pooled);
                            return Ok(msg);
                        }
                        Err((can_retry, err)) if !can_retry => return Err(err),
                        Err(_) => {
                            // 对端已静默关闭空闲连接，透明回退新建连接重试一次
                        }
                    }
                }

                let (socket_addr, _) =
                    resolve_upstream_socket_addr(&self.addr, 53, &self.connect_ip_version).await?;
                if !has_proxy {
                    check_ip_version_allowed(&socket_addr, &self.connect_ip_version)?;
                }

                let mut stream =
                    connect_via_proxy_or_direct(socket_addr, self.proxy.as_ref(), &self.connect_ip_version, timeout_duration)
                        .await?;

                let msg = exchange_framed_dns(&mut stream, &framed, timeout_duration)
                    .await
                    .map_err(|(_, e)| e)?;
                self.tcp_pool.put(stream);
                Ok(msg)
            }
            UpstreamProtocol::Dot => {
                let mut framed = Vec::with_capacity(2 + req_bytes.len());
                framed.extend_from_slice(&(req_bytes.len() as u16).to_be_bytes());
                framed.extend_from_slice(req_bytes);

                if let Some(mut pooled) = self.dot_pool.pop_fresh() {
                    match exchange_framed_dns(&mut pooled, &framed, timeout_duration).await {
                        Ok(msg) => {
                            self.dot_pool.put(pooled);
                            return Ok(msg);
                        }
                        Err((can_retry, err)) if !can_retry => return Err(err),
                        Err(_) => {
                            // 空闲 DoT 会话已被服务端断开，透明新建连接重试一次
                        }
                    }
                }

                let clean_target = self.addr.split('/').next().unwrap_or(&self.addr);
                let (socket_addr, inferred_host) =
                    resolve_upstream_socket_addr(clean_target, 853, &self.connect_ip_version).await?;
                if !has_proxy {
                    check_ip_version_allowed(&socket_addr, &self.connect_ip_version)?;
                }

                let sni_str = self
                    .host_name
                    .clone()
                    .or(inferred_host)
                    .unwrap_or_else(|| socket_addr.ip().to_string());
                let server_name = rustls_pki_types::ServerName::try_from(sni_str.as_str())
                    .map_err(|e| e.to_string())?
                    .to_owned();

                let tcp = connect_via_proxy_or_direct(
                    socket_addr,
                    self.proxy.as_ref(),
                    &self.connect_ip_version,
                    timeout_duration,
                )
                .await?;

                let connector = shared_tls_connector();
                let mut tls = timeout(timeout_duration, connector.connect(server_name, tcp))
                    .await
                    .map_err(|_| "DoT TLS handshake timeout".to_string())?
                    .map_err(|e| e.to_string())?;

                let msg = exchange_framed_dns(&mut tls, &framed, timeout_duration)
                    .await
                    .map_err(|(_, e)| e)?;
                self.dot_pool.put(tls);
                Ok(msg)
            }
            UpstreamProtocol::Doh => {
                let client = self.get_or_init_doh_client(timeout_duration)?;

                let mut final_url = self.addr.clone();

                // 若使用代理转发，为规避 Clash/Mihomo 中默认硬编码的 DOMAIN,cloudflare-dns.com,DIRECT 规则，
                // 自动将域名透明转为官方 IP 节点 1.1.1.1 (证书原生支持 1.1.1.1)，确保 100% 走代理成功连通
                let is_cf_domain = final_url.contains("cloudflare-dns.com") || final_url.contains("dns.cloudflare.com");
                if has_proxy && is_cf_domain {
                    final_url = final_url
                        .replace("cloudflare-dns.com", "1.1.1.1")
                        .replace("dns.cloudflare.com", "1.1.1.1");
                } else if let Some(ref host) = self.host_name {
                    if let Some(rest) = self.addr.strip_prefix("https://") {
                        let path = match rest.find('/') {
                            Some(idx) => &rest[idx..],
                            None => "/dns-query",
                        };
                        final_url = format!("https://{}{}", host, path);
                    }
                }

                let mut req_builder = client
                    .post(&final_url)
                    .header("content-type", "application/dns-message")
                    .header("accept", "application/dns-message")
                    .body(req_bytes.to_vec());

                if let Some(ref host) = self.host_name {
                    if !(has_proxy && (host == "cloudflare-dns.com" || host == "dns.cloudflare.com")) {
                        req_builder = req_builder.header("host", host);
                    }
                }

                let resp = req_builder
                    .send()
                    .await
                    .map_err(|e| e.to_string())?;

                if !resp.status().is_success() {
                    return Err(format!("DoH HTTP status: {}", resp.status()));
                }

                let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
                Message::from_vec(&bytes).map_err(|e| e.to_string())
            }
        }
    }
}

async fn connect_via_proxy_or_direct(
    target_addr: SocketAddr,
    proxy_opt: Option<&String>,
    connect_ip_version: &str,
    timeout_duration: Duration,
) -> Result<TcpStream, String> {
    if let Some(proxy_url) = proxy_opt {
        let p_trimmed = proxy_url.trim();
        if p_trimmed != "none" && !p_trimmed.is_empty() {
            let proxy_clean = p_trimmed
                .trim_start_matches("socks5h://")
                .trim_start_matches("socks5://")
                .trim_start_matches("http://");
            let proxy_socket = if let Ok(sa) = proxy_clean.parse::<SocketAddr>() {
                sa
            } else if let Ok(mut addrs) = tokio::net::lookup_host(proxy_clean).await {
                addrs.next().ok_or_else(|| format!("无法解析代理服务器地址: {}", proxy_clean))?
            } else {
                return Err(format!("无效的代理地址: {}", proxy_clean));
            };

            let stream_res = timeout(timeout_duration, TcpStream::connect(proxy_socket)).await;
            let mut stream = stream_res
                .map_err(|_| "Connect to SOCKS5 proxy timeout".to_string())?
                .map_err(|e| format!("Failed to connect proxy: {}", e))?;
            let _ = stream.set_nodelay(true);

            // SOCKS5 握手 1: NO AUTH (0x05 0x01 0x00)
            stream.write_all(&[0x05, 0x01, 0x00]).await.map_err(|e| e.to_string())?;
            let mut handshake_resp = [0u8; 2];
            stream.read_exact(&mut handshake_resp).await.map_err(|e| e.to_string())?;
            if handshake_resp[0] != 0x05 || handshake_resp[1] != 0x00 {
                return Err("SOCKS5 proxy authentication failed".to_string());
            }

            // SOCKS5 握手 2: CONNECT command
            let mut conn_req = vec![0x05, 0x01, 0x00];
            match target_addr {
                SocketAddr::V4(v4) => {
                    conn_req.push(0x01);
                    conn_req.extend_from_slice(&v4.ip().octets());
                    conn_req.extend_from_slice(&v4.port().to_be_bytes());
                }
                SocketAddr::V6(v6) => {
                    conn_req.push(0x04);
                    conn_req.extend_from_slice(&v6.ip().octets());
                    conn_req.extend_from_slice(&v6.port().to_be_bytes());
                }
            }
            stream.write_all(&conn_req).await.map_err(|e| e.to_string())?;

            let mut resp_header = [0u8; 4];
            stream.read_exact(&mut resp_header).await.map_err(|e| e.to_string())?;
            if resp_header[1] != 0x00 {
                return Err(format!("SOCKS5 connect command failed with code: {}", resp_header[1]));
            }
            match resp_header[3] {
                0x01 => {
                    let mut b = [0u8; 6];
                    stream.read_exact(&mut b).await.map_err(|e| e.to_string())?;
                }
                0x04 => {
                    let mut b = [0u8; 18];
                    stream.read_exact(&mut b).await.map_err(|e| e.to_string())?;
                }
                0x03 => {
                    let mut len_buf = [0u8; 1];
                    stream.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
                    let mut b = vec![0u8; len_buf[0] as usize + 2];
                    stream.read_exact(&mut b).await.map_err(|e| e.to_string())?;
                }
                _ => {}
            }
            return Ok(stream);
        }
    }

    check_ip_version_allowed(&target_addr, connect_ip_version)?;
    let stream = timeout(timeout_duration, TcpStream::connect(target_addr))
        .await
        .map_err(|_| "TCP connect timeout".to_string())?
        .map_err(|e| e.to_string())?;
    let _ = stream.set_nodelay(true);
    Ok(stream)
}

pub struct UpstreamGroup {
    pub name: String,
    pub upstreams: parking_lot::RwLock<Vec<UpstreamServer>>,
    pub timeout_ms: AtomicU64,
}

type BoxQueryFuture = Pin<Box<dyn Future<Output = Result<Message, String>> + Send>>;

impl UpstreamGroup {
    pub fn new(name: &str, upstreams: Vec<UpstreamServer>, timeout_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            upstreams: parking_lot::RwLock::new(upstreams),
            timeout_ms: AtomicU64::new(timeout_ms.max(200)),
        }
    }

    pub fn set_servers(&self, servers: Vec<UpstreamServer>) {
        *self.upstreams.write() = servers;
    }

    pub fn set_timeout_ms(&self, timeout_ms: u64) {
        let clamped = timeout_ms.max(200);
        let prev = self.timeout_ms.swap(clamped, Ordering::SeqCst);
        if prev != clamped {
            for s in self.upstreams.read().iter() {
                *s.doh_client.write() = None;
            }
        }
    }

    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.load(Ordering::Relaxed))
    }

    #[allow(dead_code)]
    pub async fn query_parallel(&self, req_bytes: &[u8]) -> Result<Message, String> {
        self.query_parallel_with_timeout(req_bytes, None).await
    }

    pub async fn query_parallel_with_timeout(
        &self,
        req_bytes: &[u8],
        timeout_override: Option<Duration>,
    ) -> Result<Message, String> {
        let servers = self.upstreams.read().clone();
        if servers.is_empty() {
            return Err(format!("Upstream group '{}' has no servers", self.name));
        }

        let now = Instant::now();
        let mut alive_indexed: Vec<(usize, UpstreamServer)> = servers
            .iter()
            .enumerate()
            .filter(|(_, s)| now >= *s.cooldown_until.read())
            .map(|(i, s)| (i, s.clone()))
            .collect();

        let group_timeout = timeout_override
            .map(|d| d.min(self.timeout_duration()).max(Duration::from_millis(100)))
            .unwrap_or_else(|| self.timeout_duration());

        // 若该分组所有节点均处于熔断冷却中（例如未开启代理时的境外分组），仅用短超时探测单个节点并快速失败
        if alive_indexed.is_empty() {
            let probe_server = servers[0].clone();
            let fast_probe_timeout = group_timeout.min(Duration::from_millis(800));
            let res = probe_server.query_internal(req_bytes, fast_probe_timeout).await;
            if res.is_ok() {
                probe_server.fail_count.store(0, Ordering::Relaxed);
                *probe_server.cooldown_until.write() = Instant::now();
            } else {
                let cnt = probe_server.fail_count.fetch_add(1, Ordering::Relaxed) + 1;
                let cooldown_secs = if cnt >= 8 { 60 } else { 30 };
                *probe_server.cooldown_until.write() = Instant::now() + Duration::from_secs(cooldown_secs);
            }
            return res.map_err(|e| format!("All upstreams in '{}' in cooldown (probe failed: {})", self.name, e));
        }

        // 按 EWMA 历史实测延迟升序排列（冷启动节点赋予 100ms 中性基线并保持用户配置优先级）
        alive_indexed.sort_by_key(|(idx, s)| {
            let ewma = s.ewma_rtt_us.load(Ordering::Relaxed);
            if ewma == 0 {
                100_000 + (*idx as u64) * 1_000
            } else {
                ewma
            }
        });
        let alive_servers: Vec<UpstreamServer> = alive_indexed.into_iter().map(|(_, s)| s).collect();

        let max_parallel = 4;
        let (first_batch, overflow_batch) = if alive_servers.len() > max_parallel {
            alive_servers.split_at(max_parallel)
        } else {
            (alive_servers.as_slice(), [].as_slice())
        };

        // 阶梯式 Hedged Request：
        // 1. 仅单个存活节点时直接请求；
        // 2. 多节点时优先向 EWMA 最快主节点发起单路请求；
        //    - 若主节点在 hedge_delay (默认 150ms，或自适应 80~250ms) 内成功返回，则零冗余流量直接完成！
        //    - 若主节点在 hedge_delay 内提前报错，立即补发备节点（零额外等待）；
        //    - 若主节点超过 hedge_delay 仍未返回，保留在途主请求并同时并发补发备节点竞速！
        let first_batch_result: Result<Message, String> = if first_batch.len() == 1 {
            first_batch[0].query(req_bytes, group_timeout).await
        } else {
            let primary = first_batch[0].clone();
            let primary_ewma_ms = primary.ewma_rtt_us.load(Ordering::Relaxed) / 1_000;
            let raw_hedge_ms = if primary_ewma_ms == 0 {
                150
            } else {
                (primary_ewma_ms * 2).clamp(80, 250)
            };
            let max_hedge_ms = (group_timeout.as_millis() as u64 / 2).max(40);
            let hedge_delay = Duration::from_millis(raw_hedge_ms.min(max_hedge_ms));

            let primary_bytes = req_bytes.to_vec();
            let mut primary_fut: BoxQueryFuture = Box::pin(async move {
                primary.query(&primary_bytes, group_timeout).await
            });

            enum PrimaryStage {
                Completed(Result<Message, String>),
                HedgeTimeout,
            }

            let stage = tokio::select! {
                res = &mut primary_fut => PrimaryStage::Completed(res),
                _ = tokio::time::sleep(hedge_delay) => PrimaryStage::HedgeTimeout,
            };

            match stage {
                PrimaryStage::Completed(Ok(msg)) => Ok(msg),
                PrimaryStage::Completed(Err(primary_err)) => {
                    // 主节点快速失败：立即并发唤醒备节点，无需空等 hedge_delay
                    let mut backup_futures: Vec<BoxQueryFuture> = Vec::with_capacity(first_batch.len() - 1);
                    for s in &first_batch[1..] {
                        let bytes_vec = req_bytes.to_vec();
                        let dur = group_timeout;
                        let s_clone = s.clone();
                        backup_futures.push(Box::pin(async move {
                            s_clone.query(&bytes_vec, dur).await
                        }));
                    }
                    select_ok(backup_futures)
                        .await
                        .map(|(msg, _)| msg)
                        .map_err(|e| format!("primary: {}; backups: {}", primary_err, e))
                }
                PrimaryStage::HedgeTimeout => {
                    // 主节点超时未回：保留在途 primary_fut，同时并发补发备节点共同竞速
                    let remaining_timeout = group_timeout
                        .saturating_sub(hedge_delay)
                        .max(Duration::from_millis(100));
                    let mut race_futures: Vec<BoxQueryFuture> = Vec::with_capacity(first_batch.len());
                    race_futures.push(primary_fut);
                    for s in &first_batch[1..] {
                        let bytes_vec = req_bytes.to_vec();
                        let dur = remaining_timeout;
                        let s_clone = s.clone();
                        race_futures.push(Box::pin(async move {
                            s_clone.query(&bytes_vec, dur).await
                        }));
                    }
                    select_ok(race_futures).await.map(|(msg, _)| msg)
                }
            }
        };

        match first_batch_result {
            Ok(msg) => Ok(msg),
            Err(first_err) => {
                if !overflow_batch.is_empty() {
                    let mut fallback_futures: Vec<BoxQueryFuture> = Vec::with_capacity(overflow_batch.len());
                    for s in overflow_batch {
                        let bytes_vec = req_bytes.to_vec();
                        let dur = group_timeout;
                        let s_clone = s.clone();
                        fallback_futures.push(Box::pin(async move {
                            s_clone.query(&bytes_vec, dur).await
                        }));
                    }
                    if let Ok((msg, _)) = select_ok(fallback_futures).await {
                        return Ok(msg);
                    }
                }
                Err(format!("All upstreams in '{}' failed: {}", self.name, first_err))
            }
        }
    }
}

pub struct ForwardPlugin {
    pub group: Arc<UpstreamGroup>,
}

impl ForwardPlugin {
    pub fn new(group: Arc<UpstreamGroup>) -> Self {
        Self { group }
    }

    pub async fn execute_with_timeout(
        &self,
        ctx: &mut QueryContext,
        timeout_override: Option<Duration>,
    ) -> PluginResult {
        let Ok(req_bytes) = ctx.request.to_vec() else {
            return PluginResult::Continue;
        };

        ctx.routing_group = Some(self.group.name.clone());

        ctx.record_timing(&format!("forward_{}_start", self.group.name));
        match self.group.query_parallel_with_timeout(&req_bytes, timeout_override).await {
            Ok(mut resp) => {
                ctx.record_timing(&format!("forward_{}_end", self.group.name));
                resp.metadata.id = ctx.request.metadata.id;
                ctx.response = Some(resp);
                ctx.tags.insert(format!("upstream:{}", self.group.name));
                PluginResult::Continue
            }
            Err(err) => {
                ctx.record_timing(&format!("forward_{}_err", self.group.name));
                ctx.tags.insert(format!("upstream_failed:{}", err));
                PluginResult::Continue
            }
        }
    }
}

#[async_trait]
impl DnsPlugin for ForwardPlugin {
    fn name(&self) -> &str {
        &self.group.name
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        self.execute_with_timeout(ctx, None).await
    }
}
