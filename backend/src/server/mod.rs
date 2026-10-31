use std::sync::Arc;
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use hickory_proto::op::Message;
use crate::db::Database;
use crate::metrics::MetricsCollector;
use crate::pipeline::context::QueryContext;
use crate::pipeline::engine::PipelineEngine;

fn bind_udp_socket(addr: SocketAddr) -> std::io::Result<UdpSocket> {
    use socket2::{Domain, Protocol, Socket, Type};
    let domain = if addr.is_ipv6() { Domain::IPV6 } else { Domain::IPV4 };
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    if addr.is_ipv6() {
        let _ = socket.set_only_v6(true);
    }
    #[cfg(unix)]
    {
        let _ = socket.set_reuse_port(true);
        let _ = socket.set_reuse_address(true);
    }
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    let std_socket: std::net::UdpSocket = socket.into();
    UdpSocket::from_std(std_socket)
}

pub struct DnsServer {
    port: u16,
    listen_mode: String,
    engine: Arc<PipelineEngine>,
    db: Arc<Database>,
    metrics: Arc<MetricsCollector>,
    pub dns_enabled: Arc<parking_lot::RwLock<bool>>,
    pub logging_enabled: Arc<parking_lot::RwLock<bool>>,
}

impl DnsServer {
    pub fn new(
        port: u16,
        listen_mode: String,
        engine: Arc<PipelineEngine>,
        db: Arc<Database>,
        metrics: Arc<MetricsCollector>,
        dns_enabled: Arc<parking_lot::RwLock<bool>>,
        logging_enabled: Arc<parking_lot::RwLock<bool>>,
    ) -> Self {
        Self {
            port,
            listen_mode,
            engine,
            db,
            metrics,
            dns_enabled,
            logging_enabled,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let mode = self.listen_mode.to_lowercase();
        let mut bind_targets: Vec<SocketAddr> = Vec::new();
        match mode.as_str() {
            "ipv4" => {
                if let Ok(sa) = format!("0.0.0.0:{}", self.port).parse() {
                    bind_targets.push(sa);
                }
            }
            "ipv6" => {
                if let Ok(sa) = format!("[::]:{}", self.port).parse() {
                    bind_targets.push(sa);
                }
            }
            _ => {
                if let Ok(v4) = format!("0.0.0.0:{}", self.port).parse() {
                    bind_targets.push(v4);
                }
                if let Ok(v6) = format!("[::]:{}", self.port).parse() {
                    bind_targets.push(v6);
                }
            }
        }

        let num_workers = std::thread::available_parallelism()
            .map(|n| n.get().clamp(2, 8))
            .unwrap_or(4);

        let inflight_semaphore = Arc::new(tokio::sync::Semaphore::new(2048));

        for &udp_addr in &bind_targets {
            match bind_udp_socket(udp_addr) {
                Ok(test_sock) => {
                    drop(test_sock);
                    tracing::info!("DNS UDP epoll SO_REUSEPORT 并发监听在 {} (启动 {} 个独立工作线程)", udp_addr, num_workers);
                    for worker_id in 0..num_workers {
                        let udp_socket = match bind_udp_socket(udp_addr) {
                            Ok(s) => Arc::new(s),
                            Err(e) => {
                                tracing::warn!("Worker {} SO_REUSEPORT bind 失败: {}, 回退常规 bind", worker_id, e);
                                match UdpSocket::bind(udp_addr).await {
                                    Ok(s) => Arc::new(s),
                                    Err(_) => continue,
                                }
                            }
                        };
                        let engine_clone = self.engine.clone();
                        let db_clone = self.db.clone();
                        let metrics_clone = self.metrics.clone();
                        let enabled_udp = self.dns_enabled.clone();
                        let logging_udp = self.logging_enabled.clone();
                        let sem = inflight_semaphore.clone();

                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 4096];
                            loop {
                                match udp_socket.recv_from(&mut buf).await {
                                    Ok((len, src)) => {
                                        if !*enabled_udp.read() {
                                            continue;
                                        }
                                        let permit = match sem.clone().try_acquire_owned() {
                                            Ok(p) => p,
                                            Err(_) => continue,
                                        };
                                        let req_bytes = buf[..len].to_vec();
                                        let socket = udp_socket.clone();
                                        let engine = engine_clone.clone();
                                        let db = db_clone.clone();
                                        let metrics = metrics_clone.clone();
                                        let logging_enabled = logging_udp.clone();

                                        tokio::spawn(async move {
                                            let _permit = permit;
                                            if let Ok(req_msg) = Message::from_vec(&req_bytes) {
                                                let mut ctx = QueryContext::new(src.ip(), req_msg);
                                                engine.process(&mut ctx).await;

                                                if let Some(ref resp_msg) = ctx.response {
                                                    if let Ok(resp_bytes) = resp_msg.to_vec() {
                                                        let _ = socket.send_to(&resp_bytes, src).await;
                                                    }
                                                }

                                                metrics.record(&ctx);
                                                if *logging_enabled.read() {
                                                    db.record_query(&ctx);
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("UDP worker {} recv error on {}: {}", worker_id, udp_addr, e);
                                    }
                                }
                            }
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("跳过无法绑定的 UDP 监听地址 {}: {}", udp_addr, e);
                }
            }
        }

        for &tcp_addr in &bind_targets {
            match TcpListener::bind(tcp_addr).await {
                Ok(tcp_listener) => {
                    tracing::info!("DNS TCP 服务监听在 {}", tcp_addr);
                    let engine_tcp = self.engine.clone();
                    let db_tcp = self.db.clone();
                    let metrics_tcp = self.metrics.clone();
                    let enabled_tcp = self.dns_enabled.clone();
                    let logging_tcp = self.logging_enabled.clone();
                    let tcp_sem = Arc::new(tokio::sync::Semaphore::new(512));

                    tokio::spawn(async move {
                        loop {
                            match tcp_listener.accept().await {
                                Ok((mut stream, src)) => {
                                    if !*enabled_tcp.read() {
                                        continue;
                                    }
                                    let permit = match tcp_sem.clone().try_acquire_owned() {
                                        Ok(p) => p,
                                        Err(_) => {
                                            tracing::warn!("DNS TCP 达到最大并发限制 (512)，拒绝来自 {} 的连接", src);
                                            continue;
                                        }
                                    };
                                    let engine = engine_tcp.clone();
                                    let db = db_tcp.clone();
                                    let metrics = metrics_tcp.clone();
                                    let logging_enabled = logging_tcp.clone();

                                    tokio::spawn(async move {
                                        let _permit = permit;
                                        let read_task = async {
                                            let mut len_buf = [0u8; 2];
                                            stream.read_exact(&mut len_buf).await?;
                                            let req_len = u16::from_be_bytes(len_buf) as usize;
                                            if req_len == 0 || req_len > 4096 {
                                                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid DNS payload length"));
                                            }
                                            let mut req_buf = vec![0u8; req_len];
                                            stream.read_exact(&mut req_buf).await?;
                                            Ok::<_, std::io::Error>(req_buf)
                                        };

                                        let req_buf = match tokio::time::timeout(std::time::Duration::from_secs(5), read_task).await {
                                            Ok(Ok(buf)) => buf,
                                            _ => return,
                                        };

                                        if let Ok(req_msg) = Message::from_vec(&req_buf) {
                                            let mut ctx = QueryContext::new(src.ip(), req_msg);
                                            engine.process(&mut ctx).await;

                                            if let Some(ref resp_msg) = ctx.response {
                                                if let Ok(resp_bytes) = resp_msg.to_vec() {
                                                    let resp_len = (resp_bytes.len() as u16).to_be_bytes();
                                                    let _ = stream.write_all(&resp_len).await;
                                                    let _ = stream.write_all(&resp_bytes).await;
                                                }
                                            }

                                            metrics.record(&ctx);
                                            if *logging_enabled.read() {
                                                db.record_query(&ctx);
                                            }
                                        }
                                    });
                                }
                                Err(e) => {
                                    tracing::error!("TCP accept error on {}: {}", tcp_addr, e);
                                }
                            }
                        }
                    });
                }
                Err(e) => {
                    tracing::warn!("跳过无法绑定的 TCP 监听地址 {}: {}", tcp_addr, e);
                }
            }
        }

        Ok(())
    }
}
