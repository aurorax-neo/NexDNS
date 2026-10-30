use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
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
}

impl UpstreamServer {
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
        })
    }

    pub async fn query(&self, req_bytes: &[u8], timeout_duration: Duration) -> Result<Message, String> {
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

                let mut stream = timeout(timeout_duration, TcpStream::connect(socket_addr))
                    .await
                    .map_err(|_| "TCP connect timeout".to_string())?
                    .map_err(|e| e.to_string())?;

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

                let tcp = timeout(timeout_duration, TcpStream::connect(socket_addr))
                    .await
                    .map_err(|_| "DoT TCP connect timeout".to_string())?
                    .map_err(|e| e.to_string())?;

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
                let mut builder = reqwest::Client::builder().timeout(timeout_duration);
                if let Some(ref p) = self.proxy {
                    if p != "none" && !p.is_empty() {
                        let mut p_addr = p.clone();
                        if !p_addr.contains("://") {
                            p_addr = format!("socks5h://{}", p_addr);
                        } else if p_addr.starts_with("socks5://") {
                            p_addr = p_addr.replacen("socks5://", "socks5h://", 1);
                        }
                        if let Ok(proxy) = reqwest::Proxy::all(&p_addr) {
                            builder = builder.proxy(proxy);
                        }
                    }
                }

                let mut final_url = self.addr.clone();

                // Handle -host-name override (e.g. https://101.226.4.6/dns-query -host-name doh.360.cn)
                if let Some(ref host) = self.host_name {
                    if let Some(rest) = self.addr.strip_prefix("https://") {
                        let (host_port_str, path) = match rest.find('/') {
                            Some(idx) => (&rest[..idx], &rest[idx..]),
                            None => (rest, "/dns-query"),
                        };
                        let (ip_str, port) = match host_port_str.split_once(':') {
                            Some((ip_part, port_part)) => (ip_part, port_part.parse().unwrap_or(443)),
                            None => (host_port_str, 443),
                        };
                        if let Ok(ip) = IpAddr::from_str(ip_str) {
                            builder = builder.resolve(host, SocketAddr::new(ip, port));
                            final_url = format!("https://{}{}", host, path);
                        }
                    }
                }

                let client = builder.build().map_err(|e| e.to_string())?;

                let resp = client
                    .post(&final_url)
                    .header("content-type", "application/dns-message")
                    .body(req_bytes.to_vec())
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
