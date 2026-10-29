use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};
use hickory_proto::op::Message;
use crate::db::Database;
use crate::metrics::MetricsCollector;
use crate::pipeline::context::QueryContext;
use crate::pipeline::engine::PipelineEngine;

pub struct DnsServer {
    port: u16,
    engine: Arc<PipelineEngine>,
    db: Arc<Database>,
    metrics: Arc<MetricsCollector>,
}

impl DnsServer {
    pub fn new(
        port: u16,
        engine: Arc<PipelineEngine>,
        db: Arc<Database>,
        metrics: Arc<MetricsCollector>,
    ) -> Self {
        Self {
            port,
            engine,
            db,
            metrics,
        }
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let udp_addr = format!("0.0.0.0:{}", self.port);
        let udp_socket = match UdpSocket::bind(&udp_addr).await {
            Ok(s) => {
                tracing::info!("DNS UDP server listening on {}", udp_addr);
                Arc::new(s)
            }
            Err(e) => {
                tracing::warn!("Failed to bind UDP port {}: {}. Attempting fallback port 5353...", self.port, e);
                let fallback = "0.0.0.0:5353";
                let s = UdpSocket::bind(fallback).await?;
                tracing::info!("DNS UDP server listening on fallback {}", fallback);
                Arc::new(s)
            }
        };

        let tcp_port = udp_socket.local_addr()?.port();
        let tcp_addr = format!("0.0.0.0:{}", tcp_port);
        let tcp_listener = TcpListener::bind(&tcp_addr).await?;
        tracing::info!("DNS TCP server listening on {}", tcp_addr);

        // Spawn UDP loop
        let udp_clone = udp_socket.clone();
        let engine_clone = self.engine.clone();
        let db_clone = self.db.clone();
        let metrics_clone = self.metrics.clone();

        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            loop {
                match udp_clone.recv_from(&mut buf).await {
                    Ok((len, src)) => {
                        let req_bytes = buf[..len].to_vec();
                        let socket = udp_clone.clone();
                        let engine = engine_clone.clone();
                        let db = db_clone.clone();
                        let metrics = metrics_clone.clone();

                        tokio::spawn(async move {
                            if let Ok(req_msg) = Message::from_vec(&req_bytes) {
                                let mut ctx = QueryContext::new(src.ip(), req_msg);
                                engine.process(&mut ctx).await;
                                metrics.record(&ctx);
                                db.record_query(&ctx);

                                if let Some(ref resp_msg) = ctx.response {
                                    if let Ok(resp_bytes) = resp_msg.to_vec() {
                                        let _ = socket.send_to(&resp_bytes, src).await;
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("UDP recv error: {}", e);
                    }
                }
            }
        });

        // Spawn TCP loop
        let engine_tcp = self.engine.clone();
        let db_tcp = self.db.clone();
        let metrics_tcp = self.metrics.clone();

        tokio::spawn(async move {
            loop {
                match tcp_listener.accept().await {
                    Ok((mut stream, src)) => {
                        let engine = engine_tcp.clone();
                        let db = db_tcp.clone();
                        let metrics = metrics_tcp.clone();

                        tokio::spawn(async move {
                            let mut len_buf = [0u8; 2];
                            if stream.read_exact(&mut len_buf).await.is_err() {
                                return;
                            }
                            let req_len = u16::from_be_bytes(len_buf) as usize;
                            let mut req_buf = vec![0u8; req_len];
                            if stream.read_exact(&mut req_buf).await.is_err() {
                                return;
                            }

                            if let Ok(req_msg) = Message::from_vec(&req_buf) {
                                let mut ctx = QueryContext::new(src.ip(), req_msg);
                                engine.process(&mut ctx).await;
                                metrics.record(&ctx);
                                db.record_query(&ctx);

                                if let Some(ref resp_msg) = ctx.response {
                                    if let Ok(resp_bytes) = resp_msg.to_vec() {
                                        let resp_len = (resp_bytes.len() as u16).to_be_bytes();
                                        let _ = stream.write_all(&resp_len).await;
                                        let _ = stream.write_all(&resp_bytes).await;
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("TCP accept error: {}", e);
                    }
                }
            }
        });

        Ok(())
    }
}
