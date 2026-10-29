use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use parking_lot::RwLock;
use tokio::net::TcpStream;
use tokio::time::timeout;
use hickory_proto::rr::{RData, Record};
use crate::pipeline::context::QueryContext;
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

#[derive(Clone)]
struct IpScore {
    rtt_ms: u64,
    tested_at: Instant,
}

pub struct SpeedCheckPlugin {
    cache: Arc<RwLock<HashMap<IpAddr, IpScore>>>,
    cache_ttl: Duration,
    probe_timeout: Duration,
    target_ports: Vec<u16>,
    enabled: bool,
}

impl SpeedCheckPlugin {
    pub fn new(enabled: bool, timeout_ms: u64, cache_mins: u64) -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_ttl: Duration::from_secs(cache_mins * 60),
            probe_timeout: Duration::from_millis(timeout_ms),
            target_ports: vec![80, 443],
            enabled,
        }
    }

    async fn test_ip(&self, ip: IpAddr) -> u64 {
        // Check cache first
        {
            let r = self.cache.read();
            if let Some(score) = r.get(&ip) {
                if score.tested_at.elapsed() < self.cache_ttl {
                    return score.rtt_ms;
                }
            }
        }

        let mut best_rtt = 9999u64;
        for port in &self.target_ports {
            let socket_addr = SocketAddr::new(ip, *port);
            let start = Instant::now();
            if let Ok(Ok(_)) = timeout(self.probe_timeout, TcpStream::connect(socket_addr)).await {
                let rtt = start.elapsed().as_millis() as u64;
                if rtt < best_rtt {
                    best_rtt = rtt;
                }
            }
        }

        // Cache result
        {
            let mut w = self.cache.write();
            w.insert(
                ip,
                IpScore {
                    rtt_ms: best_rtt,
                    tested_at: Instant::now(),
                },
            );
        }

        best_rtt
    }
}

#[async_trait]
impl DnsPlugin for SpeedCheckPlugin {
    fn name(&self) -> &str {
        "speed_check"
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        if !self.enabled {
            return PluginResult::Continue;
        }

        let Some(ref mut resp) = ctx.response else {
            return PluginResult::Continue;
        };

        let mut ip_records: Vec<(IpAddr, Record)> = Vec::new();
        let mut other_records: Vec<Record> = Vec::new();

        for record in std::mem::take(&mut resp.answers) {
            match &record.data {
                RData::A(a) => {
                    ip_records.push((IpAddr::V4(a.0), record));
                }
                RData::AAAA(aaaa) => {
                    ip_records.push((IpAddr::V6(aaaa.0), record));
                }
                _ => {
                    other_records.push(record);
                }
            }
        }

        if ip_records.len() <= 1 {
            // Only 0 or 1 IP, no re-ordering needed
            for (ip, r) in ip_records {
                ctx.fastest_ip = Some(ip);
                resp.answers.push(r);
            }
            for r in other_records {
                resp.answers.push(r);
            }
            return PluginResult::Continue;
        }

        // Concurrently probe all IPs
        let mut tasks = Vec::new();
        for (ip, record) in ip_records {
            let self_clone = SpeedCheckPlugin {
                cache: self.cache.clone(),
                cache_ttl: self.cache_ttl,
                probe_timeout: self.probe_timeout,
                target_ports: self.target_ports.clone(),
                enabled: self.enabled,
            };
            tasks.push(tokio::spawn(async move {
                let rtt = self_clone.test_ip(ip).await;
                (ip, record, rtt)
            }));
        }

        let mut scored_records = Vec::new();
        for t in tasks {
            if let Ok(res) = t.await {
                scored_records.push(res);
            }
        }

        // Sort by RTT ascending (fastest first)
        scored_records.sort_by_key(|(_, _, rtt)| *rtt);

        if let Some((best_ip, _, best_rtt)) = scored_records.first() {
            ctx.fastest_ip = Some(*best_ip);
            ctx.tags.insert(format!("fastest_ip:{}:{}ms", best_ip, best_rtt));
        }

        for (_, record, _) in scored_records {
            resp.answers.push(record);
        }
        for record in other_records {
            resp.answers.push(record);
        }

        PluginResult::Continue
    }
}
