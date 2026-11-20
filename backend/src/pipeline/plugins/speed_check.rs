use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use futures_util::future::select_ok;
use parking_lot::RwLock;
use tokio::net::TcpStream;
use tokio::sync::Semaphore;
use tokio::time::timeout;
use hickory_proto::rr::{RData, Record};
use crate::pipeline::context::QueryContext;
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

pub const MAX_SPEED_CACHE_ENTRIES: usize = 8192;
const MAX_CONCURRENT_PROBES: usize = 16;

#[derive(Clone)]
struct IpScore {
    rtt_ms: u64,
    tested_at: Instant,
}

pub struct SpeedCheckPlugin {
    cache: Arc<RwLock<HashMap<IpAddr, IpScore>>>,
    max_cache_entries: usize,
    cache_ttl_secs: AtomicU64,
    probe_timeout_ms: AtomicU64,
    target_ports: RwLock<Vec<u16>>,
    pub direct_only: AtomicBool,
    connect_ip_version: RwLock<String>,
    probe_semaphore: Arc<Semaphore>,
    pub enabled: Arc<AtomicBool>,
}

impl SpeedCheckPlugin {
    #[allow(dead_code)]
    pub fn new(enabled: bool, timeout_ms: u64, cache_mins: u64) -> Self {
        Self::with_options(enabled, timeout_ms, cache_mins, "tcp_80_443", true, "dual")
    }

    pub fn with_options(
        enabled: bool,
        timeout_ms: u64,
        cache_mins: u64,
        mode: &str,
        direct_only: bool,
        connect_ip_version: &str,
    ) -> Self {
        Self::with_capacity(
            enabled,
            timeout_ms,
            cache_mins,
            mode,
            direct_only,
            connect_ip_version,
            MAX_SPEED_CACHE_ENTRIES,
        )
    }

    pub fn with_capacity(
        enabled: bool,
        timeout_ms: u64,
        cache_mins: u64,
        mode: &str,
        direct_only: bool,
        connect_ip_version: &str,
        max_cache_entries: usize,
    ) -> Self {
        let cap = max_cache_entries.max(4);
        Self {
            cache: Arc::new(RwLock::new(HashMap::with_capacity(cap.min(1024)))),
            max_cache_entries: cap,
            cache_ttl_secs: AtomicU64::new(cache_mins.max(1) * 60),
            probe_timeout_ms: AtomicU64::new(timeout_ms.clamp(50, 10000)),
            target_ports: RwLock::new(Self::parse_mode_ports(mode)),
            direct_only: AtomicBool::new(direct_only),
            connect_ip_version: RwLock::new(normalize_ip_ver(connect_ip_version)),
            probe_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_PROBES)),
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn parse_mode_ports(mode: &str) -> Vec<u16> {
        let lower = mode.trim().to_ascii_lowercase();
        match lower.as_str() {
            "https" | "tcp_443" | "443" => return vec![443],
            "http" | "tcp_80" | "80" => return vec![80],
            "tcp_80_443" | "" => return vec![80, 443],
            _ => {}
        }

        let mut ports = Vec::new();
        for token in lower.split(|c: char| !c.is_ascii_digit()) {
            if token.is_empty() {
                continue;
            }
            if let Ok(p) = token.parse::<u16>() {
                if p > 0 && !ports.contains(&p) {
                    ports.push(p);
                }
            }
        }
        if ports.is_empty() {
            vec![80, 443]
        } else {
            ports
        }
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::SeqCst);
    }

    pub fn update_config(
        &self,
        enabled: bool,
        timeout_ms: u64,
        cache_mins: u64,
        mode: &str,
        direct_only: bool,
        connect_ip_version: &str,
    ) {
        self.enabled.store(enabled, Ordering::SeqCst);
        self.probe_timeout_ms.store(timeout_ms.clamp(50, 10000), Ordering::SeqCst);
        self.cache_ttl_secs.store(cache_mins.max(1) * 60, Ordering::SeqCst);
        *self.target_ports.write() = Self::parse_mode_ports(mode);
        self.direct_only.store(direct_only, Ordering::SeqCst);
        *self.connect_ip_version.write() = normalize_ip_ver(connect_ip_version);
    }

    #[allow(dead_code)]
    pub fn target_ports_snapshot(&self) -> Vec<u16> {
        self.target_ports.read().clone()
    }

    #[allow(dead_code)]
    pub fn probe_timeout_ms(&self) -> u64 {
        self.probe_timeout_ms.load(Ordering::Relaxed)
    }

    #[allow(dead_code)]
    pub fn cache_size(&self) -> usize {
        self.cache.read().len()
    }

    #[allow(dead_code)]
    pub fn insert_score_for_test(&self, ip: IpAddr, rtt_ms: u64) {
        let cache_ttl = Duration::from_secs(self.cache_ttl_secs.load(Ordering::Relaxed));
        insert_bounded_score(&self.cache, self.max_cache_entries, cache_ttl, ip, rtt_ms);
    }
}

fn normalize_ip_ver(v: &str) -> String {
    match v.trim().to_ascii_lowercase().as_str() {
        "ipv4" | "ipv4_only" => "ipv4".to_string(),
        "ipv6" | "ipv6_only" => "ipv6".to_string(),
        _ => "dual".to_string(),
    }
}

fn ip_allowed_by_version(ip: &IpAddr, ver: &str) -> bool {
    match ver {
        "ipv4" => ip.is_ipv4(),
        "ipv6" => ip.is_ipv6(),
        _ => true,
    }
}

fn insert_bounded_score(
    cache: &RwLock<HashMap<IpAddr, IpScore>>,
    max_entries: usize,
    cache_ttl: Duration,
    ip: IpAddr,
    rtt_ms: u64,
) {
    let mut w = cache.write();
    if !w.contains_key(&ip) && w.len() >= max_entries {
        // 1. 先清理已过期的测速记录
        w.retain(|_, score| score.tested_at.elapsed() < cache_ttl);

        // 2. 若仍在上限之上，按抽样最旧策略淘汰，杜绝路由器小内存容器长期运行泄漏
        if w.len() >= max_entries {
            let evict_count = (max_entries / 32).clamp(1, 32);
            let mut sample: Vec<(IpAddr, Instant)> = w
                .iter()
                .take(64)
                .map(|(k, v)| (*k, v.tested_at))
                .collect();
            sample.sort_by_key(|(_, ts)| *ts);
            for (k, _) in sample.into_iter().take(evict_count) {
                w.remove(&k);
            }
        }
    }

    w.insert(
        ip,
        IpScore {
            rtt_ms,
            tested_at: Instant::now(),
        },
    );
}

async fn probe_ip_rtt(
    cache: Arc<RwLock<HashMap<IpAddr, IpScore>>>,
    max_entries: usize,
    ip: IpAddr,
    ports: &[u16],
    probe_timeout: Duration,
    cache_ttl: Duration,
) -> u64 {
    {
        let r = cache.read();
        if let Some(score) = r.get(&ip) {
            if score.tested_at.elapsed() < cache_ttl {
                return score.rtt_ms;
            }
        }
    }

    let mut port_probes = Vec::with_capacity(ports.len());
    for &port in ports {
        let socket_addr = SocketAddr::new(ip, port);
        port_probes.push(Box::pin(async move {
            let start = Instant::now();
            match timeout(probe_timeout, TcpStream::connect(socket_addr)).await {
                Ok(Ok(_stream)) => Ok((start.elapsed().as_millis() as u64).max(1)),
                _ => Err(()),
            }
        }));
    }

    let best_rtt = if port_probes.is_empty() {
        9999u64
    } else {
        match select_ok(port_probes).await {
            Ok((rtt, _)) => rtt,
            Err(_) => 9999u64,
        }
    };

    insert_bounded_score(&cache, max_entries, cache_ttl, ip, best_rtt);
    best_rtt
}

#[async_trait]
impl DnsPlugin for SpeedCheckPlugin {
    fn name(&self) -> &str {
        "speed_check"
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        if !self.enabled.load(Ordering::Relaxed) {
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

        // 跳过境外/代理组直连 TCP 测速（直连探测境外被墙/代理目标只会超时发废包），但仍严格保持 CNAME 置顶规范
        let is_proxy_routed = matches!(ctx.routing_group.as_deref(), Some("gfw") | Some("overseas"))
            || ctx.tags.contains("route:proxy")
            || ctx.tags.contains("route:custom_proxy")
            || ctx.tags.contains("fallback_to_gfw");

        if ip_records.len() <= 1 || (self.direct_only.load(Ordering::Relaxed) && is_proxy_routed) {
            // CRITICAL (RFC 1034 / RFC 2181): CNAME 别名重定向链必须严格位于应答区首部！
            for r in other_records {
                resp.answers.push(r);
            }
            for (idx, (ip, r)) in ip_records.into_iter().enumerate() {
                if idx == 0 {
                    ctx.fastest_ip = Some(ip);
                }
                resp.answers.push(r);
            }
            if is_proxy_routed {
                ctx.tags.insert("speed_check_skipped:proxy_group".to_string());
            }
            return PluginResult::Continue;
        }

        let cache_ttl = Duration::from_secs(self.cache_ttl_secs.load(Ordering::Relaxed));
        let probe_timeout = Duration::from_millis(self.probe_timeout_ms.load(Ordering::Relaxed));
        let mut all_cached = true;
        let mut cached_scores = Vec::new();
        {
            let r = self.cache.read();
            for (ip, _) in &ip_records {
                if let Some(score) = r.get(ip) {
                    if score.tested_at.elapsed() < cache_ttl {
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
            // 非阻塞异步后台测速：严格尊重用户配置的 timeout_ms，不再硬编码截断为 180ms；
            // 同时通过全局并发信号量限制最多 16 路在途探测，防止高并发下耗尽端口/句柄。
            let ip_ver = self.connect_ip_version.read().clone();
            let probe_limit = ip_records.len().min(3);
            let ips_to_probe: Vec<IpAddr> = ip_records
                .iter()
                .filter(|(ip, _)| ip_allowed_by_version(ip, &ip_ver))
                .take(probe_limit)
                .map(|(ip, _)| *ip)
                .collect();

            if !ips_to_probe.is_empty() {
                let cache = self.cache.clone();
                let max_entries = self.max_cache_entries;
                let ports = self.target_ports.read().clone();
                let p_timeout = probe_timeout;
                let c_ttl = cache_ttl;
                let sem = self.probe_semaphore.clone();
                tokio::spawn(async move {
                    for ip in ips_to_probe {
                        let Ok(_permit) = sem.clone().try_acquire_owned() else {
                            break;
                        };
                        let _ = probe_ip_rtt(cache.clone(), max_entries, ip, &ports, p_timeout, c_ttl).await;
                    }
                });
            }

            let r = self.cache.read();
            for (idx, (ip, rec)) in ip_records.into_iter().enumerate() {
                let rtt = r
                    .get(&ip)
                    .filter(|s| s.tested_at.elapsed() < cache_ttl)
                    .map(|s| s.rtt_ms)
                    .unwrap_or(5000 + idx as u64);
                scored_records.push((ip, rec, rtt));
            }
        }

        scored_records.sort_by_key(|(_, _, rtt)| *rtt);

        if let Some((best_ip, _, best_rtt)) = scored_records.first() {
            ctx.fastest_ip = Some(*best_ip);
            ctx.tags.insert(format!("fastest_ip:{}:{}ms", best_ip, best_rtt));
        }

        for record in other_records {
            resp.answers.push(record);
        }
        for (_, record, _) in scored_records {
            resp.answers.push(record);
        }

        PluginResult::Continue
    }
}
