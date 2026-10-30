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

    #[allow(dead_code)]
    async fn test_ip(&self, ip: IpAddr) -> u64 {
        probe_ip_rtt(
            self.cache.clone(),
            ip,
            &self.target_ports,
            self.probe_timeout,
            self.cache_ttl,
        ).await
    }
}

async fn probe_ip_rtt(
    cache: Arc<RwLock<HashMap<IpAddr, IpScore>>>,
    ip: IpAddr,
    ports: &[u16],
    probe_timeout: Duration,
    cache_ttl: Duration,
) -> u64 {
    // Check cache first
    {
        let r = cache.read();
        if let Some(score) = r.get(&ip) {
            if score.tested_at.elapsed() < cache_ttl {
                return score.rtt_ms;
            }
        }
    }

    let mut best_rtt = 9999u64;
    for port in ports {
        let socket_addr = SocketAddr::new(ip, *port);
        let start = Instant::now();
        if let Ok(Ok(_)) = timeout(probe_timeout, TcpStream::connect(socket_addr)).await {
            let rtt = start.elapsed().as_millis() as u64;
            if rtt < best_rtt {
                best_rtt = rtt;
            }
        }
    }

    // Cache result
    {
        let mut w = cache.write();
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
            // CRITICAL (RFC 1034 / RFC 2181): CNAME 别名重定向链必须严格位于应答区首部！
            // 若 IP 记录排在 CNAME 之前，操作系统与浏览器解析器（glibc, macOS, Windows, Chrome）
            // 将无法识别 CNAME 目标对应关系，导致报错 "Name or service not known" 或网页无法打开。
            for r in other_records {
                resp.answers.push(r);
            }
            for (ip, r) in ip_records {
                ctx.fastest_ip = Some(ip);
                resp.answers.push(r);
            }
            return PluginResult::Continue;
        }

        // Fast path: check in-memory cache first without spawning tasks
        let mut all_cached = true;
        let mut cached_scores = Vec::new();
        {
            let r = self.cache.read();
            for (ip, _) in &ip_records {
                if let Some(score) = r.get(ip) {
                    if score.tested_at.elapsed() < self.cache_ttl {
                        cached_scores.push((*ip, score.rtt_ms));
                    } else {
                        all_cached = false;
                        break;
                    }
                } else {
                    all_cached = false;
                    break;
                }
            }
        }

        let mut scored_records = Vec::new();
        if all_cached && cached_scores.len() == ip_records.len() {
            for (i, (ip, rec)) in ip_records.into_iter().enumerate() {
                scored_records.push((ip, rec, cached_scores[i].1));
            }
        } else {
            // Limit concurrent probing to top 3 IPs to prevent TCP socket explosion
            let probe_limit = ip_records.len().min(3);
            let (to_probe, remaining) = ip_records.split_at(probe_limit);

            let mut tasks = Vec::new();
            for (ip, record) in to_probe.iter().cloned() {
                let cache = self.cache.clone();
                let ports = self.target_ports.clone();
                let p_timeout = self.probe_timeout.min(Duration::from_millis(200));
                let c_ttl = self.cache_ttl;
                tasks.push(tokio::spawn(async move {
                    let rtt = probe_ip_rtt(cache, ip, &ports, p_timeout, c_ttl).await;
                    (ip, record, rtt)
                }));
            }

            for t in tasks {
                if let Ok(res) = t.await {
                    scored_records.push(res);
                }
            }
            for (ip, rec) in remaining.iter().cloned() {
                scored_records.push((ip, rec, 9999));
            }
        }

        // Sort by RTT ascending (fastest first)
        scored_records.sort_by_key(|(_, _, rtt)| *rtt);

        if let Some((best_ip, _, best_rtt)) = scored_records.first() {
            ctx.fastest_ip = Some(*best_ip);
            ctx.tags.insert(format!("fastest_ip:{}:{}ms", best_ip, best_rtt));
        }

        // CNAME 别名记录与非 IP 记录必须优先放置于应答首部，紧接着才是优选排序后的 IP 记录
        for record in other_records {
            resp.answers.push(record);
        }
        for (_, record, _) in scored_records {
            resp.answers.push(record);
        }

        PluginResult::Continue
    }
}
