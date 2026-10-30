use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::str::FromStr;
use async_trait::async_trait;
use futures_util::future::select_ok;
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;
use hickory_proto::op::Message;
use crate::pipeline::context::QueryContext;
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum UpstreamProtocol {
    Udp,
    Tcp,
    Dot,
    Doh,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpstreamServer {
    pub tag: String,
    pub addr: String,
    pub protocol: UpstreamProtocol,
    pub bootstrap_ip: Option<String>,
    pub host_name: Option<String>,
    pub proxy: Option<String>,
    #[serde(skip)]
    pub doh_client: Arc<parking_lot::RwLock<Option<reqwest::Client>>>,
    #[serde(skip, default)]
    pub fail_count: Arc<std::sync::atomic::AtomicU32>,
    #[serde(skip, default = "default_cooldown")]
    pub cooldown_until: Arc<parking_lot::RwLock<Instant>>,
}

fn default_cooldown() -> Arc<parking_lot::RwLock<Instant>> {
    Arc::new(parking_lot::RwLock::new(Instant::now()))
}

impl UpstreamServer {
    pub fn set_proxy(&mut self, proxy: Option<String>) {
        self.proxy = proxy;
        *self.doh_client.write() = None;
        self.fail_count.store(0, std::sync::atomic::Ordering::Relaxed);
        *self.cooldown_until.write() = Instant::now();
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
            .tcp_keepalive(Some(Duration::from_secs(60)));

        if let Some(ref p) = self.proxy {
            if p != "none" && !p.is_empty() {
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

        // Handle -host-name override (e.g. https://101.226.4.6/dns-query -host-name doh.360.cn)
        if let Some(ref host) = self.host_name {
            if let Some(rest) = self.addr.strip_prefix("https://") {
                let (host_port_str, _path) = match rest.find('/') {
                    Some(idx) => (&rest[..idx], &rest[idx..]),
                    None => (rest, "/dns-query"),
                };
                let (ip_str, port) = match host_port_str.split_once(':') {
                    Some((ip_part, port_part)) => (ip_part, port_part.parse().unwrap_or(443)),
                    None => (host_port_str, 443),
                };
                if let Ok(ip) = IpAddr::from_str(ip_str) {
                    builder = builder.resolve(host, SocketAddr::new(ip, port));
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
            } else if part.starts_with("-") {
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
        // or e.g. https://101.226.4.6/dns-query#doh.360.cn
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
            doh_client: Arc::new(parking_lot::RwLock::new(None)),
            fail_count: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            cooldown_until: Arc::new(parking_lot::RwLock::new(Instant::now())),
        })
    }

    pub async fn query(&self, req_bytes: &[u8], timeout_duration: Duration) -> Result<Message, String> {
        let now = Instant::now();
        if now < *self.cooldown_until.read() {
            return Err(format!("上游节点 [{}] 熔断冷却中", self.addr));
        }

        let res = self.query_internal(req_bytes, timeout_duration).await;
        match &res {
            Ok(_) => {
                self.fail_count.store(0, std::sync::atomic::Ordering::Relaxed);
            }
            Err(_) => {
                let cnt = self.fail_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                if cnt >= 3 {
                    let mut w = self.cooldown_until.write();
                    *w = Instant::now() + Duration::from_secs(10);
                }
            }
        }
        res
    }

    async fn query_internal(&self, req_bytes: &[u8], timeout_duration: Duration) -> Result<Message, String> {
        match self.protocol {
            UpstreamProtocol::Udp => {
                let socket_addr: SocketAddr = if self.addr.contains(':') {
                    self.addr.parse().map_err(|e| format!("Invalid UDP addr: {}", e))?
                } else {
                    format!("{}:53", self.addr).parse().map_err(|e| format!("Invalid UDP addr: {}", e))?
                };

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
                let socket_addr: SocketAddr = if self.addr.contains(':') {
                    self.addr.parse().map_err(|e| format!("Invalid TCP addr: {}", e))?
                } else {
                    format!("{}:53", self.addr).parse().map_err(|e| format!("Invalid TCP addr: {}", e))?
                };

                let mut stream = connect_via_proxy_or_direct(socket_addr, self.proxy.as_ref(), timeout_duration).await?;

                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let len_bytes = (req_bytes.len() as u16).to_be_bytes();
                stream.write_all(&len_bytes).await.map_err(|e| e.to_string())?;
                stream.write_all(req_bytes).await.map_err(|e| e.to_string())?;

                let mut resp_len_buf = [0u8; 2];
                timeout(timeout_duration, stream.read_exact(&mut resp_len_buf))
                    .await
                    .map_err(|_| "TCP read len timeout".to_string())?
                    .map_err(|e| e.to_string())?;
                let resp_len = u16::from_be_bytes(resp_len_buf) as usize;

                let mut resp_buf = vec![0u8; resp_len];
                timeout(timeout_duration, stream.read_exact(&mut resp_buf))
                    .await
                    .map_err(|_| "TCP read body timeout".to_string())?
                    .map_err(|e| e.to_string())?;

                Message::from_vec(&resp_buf).map_err(|e| e.to_string())
            }
            UpstreamProtocol::Dot => {
                let socket_addr: SocketAddr = if self.addr.contains(':') {
                    self.addr.parse().map_err(|e| format!("Invalid DoT addr: {}", e))?
                } else {
                    format!("{}:853", self.addr).parse().map_err(|e| format!("Invalid DoT addr: {}", e))?
                };

                let root_store = rustls::RootCertStore {
                    roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
                };
                let client_config = rustls::ClientConfig::builder()
                    .with_root_certificates(root_store)
                    .with_no_client_auth();
                let connector = tokio_rustls::TlsConnector::from(Arc::new(client_config));

                let server_name = match &self.host_name {
                    Some(h) => rustls_pki_types::ServerName::try_from(h.as_str())
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                    None => rustls_pki_types::ServerName::try_from(socket_addr.ip().to_string().as_str())
                        .map_err(|e| e.to_string())?
                        .to_owned(),
                };

                let tcp = connect_via_proxy_or_direct(socket_addr, self.proxy.as_ref(), timeout_duration).await?;

                let mut tls = timeout(timeout_duration, connector.connect(server_name, tcp))
                    .await
                    .map_err(|_| "DoT TLS handshake timeout".to_string())?
                    .map_err(|e| e.to_string())?;

                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let len_bytes = (req_bytes.len() as u16).to_be_bytes();
                tls.write_all(&len_bytes).await.map_err(|e| e.to_string())?;
                tls.write_all(req_bytes).await.map_err(|e| e.to_string())?;

                let mut resp_len_buf = [0u8; 2];
                timeout(timeout_duration, tls.read_exact(&mut resp_len_buf))
                    .await
                    .map_err(|_| "DoT read len timeout".to_string())?
                    .map_err(|e| e.to_string())?;
                let resp_len = u16::from_be_bytes(resp_len_buf) as usize;

                let mut resp_buf = vec![0u8; resp_len];
                timeout(timeout_duration, tls.read_exact(&mut resp_buf))
                    .await
                    .map_err(|_| "DoT read body timeout".to_string())?
                    .map_err(|e| e.to_string())?;

                Message::from_vec(&resp_buf).map_err(|e| e.to_string())
            }
            UpstreamProtocol::Doh => {
                let client = self.get_or_init_doh_client(timeout_duration)?;
                let has_proxy = self.proxy.as_deref().map(|p| p != "none" && !p.is_empty()).unwrap_or(false);

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
            {
                let stream_res = timeout(timeout_duration, TcpStream::connect(proxy_socket)).await;
                let mut stream = stream_res
                    .map_err(|_| "Connect to SOCKS5 proxy timeout".to_string())?
                    .map_err(|e| format!("Failed to connect proxy: {}", e))?;

                use tokio::io::{AsyncReadExt, AsyncWriteExt};
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
                // 跳过绑定的代理应答地址
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
    }

    // 直连
    let stream = timeout(timeout_duration, TcpStream::connect(target_addr))
        .await
        .map_err(|_| "TCP connect timeout".to_string())?
        .map_err(|e| e.to_string())?;
    Ok(stream)
}

pub struct UpstreamGroup {
    pub name: String,
    pub upstreams: parking_lot::RwLock<Vec<UpstreamServer>>,
    pub timeout: Duration,
}

impl UpstreamGroup {
    pub fn new(name: &str, upstreams: Vec<UpstreamServer>, timeout_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            upstreams: parking_lot::RwLock::new(upstreams),
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    pub fn set_servers(&self, servers: Vec<UpstreamServer>) {
        *self.upstreams.write() = servers;
    }

    pub async fn query_parallel(&self, req_bytes: &[u8]) -> Result<Message, String> {
        let servers = self.upstreams.read().clone();
        if servers.is_empty() {
            return Err(format!("Upstream group '{}' has no servers", self.name));
        }

        let mut futures = Vec::new();
        for s in servers {
            let bytes_vec = req_bytes.to_vec();
            let dur = self.timeout;
            futures.push(Box::pin(async move {
                s.query(&bytes_vec, dur).await
            }));
        }

        match select_ok(futures).await {
            Ok((msg, _)) => Ok(msg),
            Err(e) => Err(format!("All upstreams in '{}' failed: {}", self.name, e)),
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
}

#[async_trait]
impl DnsPlugin for ForwardPlugin {
    fn name(&self) -> &str {
        &self.group.name
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        let Ok(req_bytes) = ctx.request.to_vec() else {
            return PluginResult::Continue;
        };

        ctx.record_timing(&format!("forward_{}_start", self.group.name));
        match self.group.query_parallel(&req_bytes).await {
            Ok(mut resp) => {
                ctx.record_timing(&format!("forward_{}_end", self.group.name));
                resp.metadata.id = ctx.request.metadata.id;
                ctx.response = Some(resp);
                ctx.routing_group = Some(self.group.name.clone());
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
