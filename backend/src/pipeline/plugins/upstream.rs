use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
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
    Doh,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpstreamServer {
    pub tag: String,
    pub addr: String,
    pub protocol: UpstreamProtocol,
    pub bootstrap_ip: Option<String>,
}

impl UpstreamServer {
    pub fn parse(tag: &str, uri: &str) -> Option<Self> {
        if let Some(rest) = uri.strip_prefix("https://") {
            Some(Self {
                tag: tag.to_string(),
                addr: format!("https://{}", rest),
                protocol: UpstreamProtocol::Doh,
                bootstrap_ip: None,
            })
        } else if let Some(rest) = uri.strip_prefix("tcp://") {
            Some(Self {
                tag: tag.to_string(),
                addr: rest.to_string(),
                protocol: UpstreamProtocol::Tcp,
                bootstrap_ip: None,
            })
        } else if let Some(rest) = uri.strip_prefix("udp://") {
            Some(Self {
                tag: tag.to_string(),
                addr: rest.to_string(),
                protocol: UpstreamProtocol::Udp,
                bootstrap_ip: None,
            })
        } else {
            Some(Self {
                tag: tag.to_string(),
                addr: uri.to_string(),
                protocol: UpstreamProtocol::Udp,
                bootstrap_ip: None,
            })
        }
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
            UpstreamProtocol::Doh => {
                let client = reqwest::Client::builder()
                    .timeout(timeout_duration)
                    .build()
                    .map_err(|e| e.to_string())?;

                let resp = client
                    .post(&self.addr)
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
    pub upstreams: Vec<UpstreamServer>,
    pub timeout: Duration,
}

impl UpstreamGroup {
    pub fn new(name: &str, upstreams: Vec<UpstreamServer>, timeout_ms: u64) -> Self {
        Self {
            name: name.to_string(),
            upstreams,
            timeout: Duration::from_millis(timeout_ms),
        }
    }

    pub async fn query_parallel(&self, req_bytes: &[u8]) -> Result<Message, String> {
        if self.upstreams.is_empty() {
            return Err(format!("Upstream group '{}' has no servers", self.name));
        }

        let mut futures = Vec::new();
        for s in &self.upstreams {
            let s_clone = s.clone();
            let bytes_vec = req_bytes.to_vec();
            let dur = self.timeout;
            futures.push(Box::pin(async move {
                s_clone.query(&bytes_vec, dur).await
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
