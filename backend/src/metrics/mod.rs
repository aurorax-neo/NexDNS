use chrono::{Local, Timelike};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use crate::pipeline::context::{QueryContext, QueryStatus};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheStatRow {
    pub requests: u64,
    pub hits: u64,
    pub misses: u64,
    pub hit_rate: f64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CacheStatsSnapshot {
    pub yesterday: CacheStatRow,
    pub today: CacheStatRow,
    pub total: CacheStatRow,
    pub last_24h: CacheStatRow,
    pub updated_at: String,
}

#[derive(Default, Clone, Serialize, Deserialize)]
struct HourlyCacheSlot {
    epoch_hour: u64,
    requests: u64,
    hits: u64,
}

pub struct CacheStatsTracker {
    current_date: RwLock<String>,
    today_requests: AtomicU64,
    today_hits: AtomicU64,
    yesterday_requests: AtomicU64,
    yesterday_hits: AtomicU64,
    total_requests: AtomicU64,
    total_hits: AtomicU64,
    hourly_slots: RwLock<Vec<HourlyCacheSlot>>,
}

impl CacheStatsTracker {
    pub fn new() -> Arc<Self> {
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        Arc::new(Self {
            current_date: RwLock::new(today_str),
            today_requests: AtomicU64::new(0),
            today_hits: AtomicU64::new(0),
            yesterday_requests: AtomicU64::new(0),
            yesterday_hits: AtomicU64::new(0),
            total_requests: AtomicU64::new(0),
            total_hits: AtomicU64::new(0),
            hourly_slots: RwLock::new((0..24).map(|_| HourlyCacheSlot::default()).collect()),
        })
    }

    pub fn check_rollover(&self, today_str: &str) {
        if *self.current_date.read() != today_str {
            let mut cur = self.current_date.write();
            if *cur != today_str {
                if !cur.is_empty() {
                    let t_req = self.today_requests.swap(0, Ordering::SeqCst);
                    let t_hit = self.today_hits.swap(0, Ordering::SeqCst);
                    self.yesterday_requests.store(t_req, Ordering::SeqCst);
                    self.yesterday_hits.store(t_hit, Ordering::SeqCst);
                }
                *cur = today_str.to_string();
            }
        }
    }

    pub fn record(&self, is_hit: bool) {
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        self.check_rollover(&today_str);

        self.today_requests.fetch_add(1, Ordering::Relaxed);
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        if is_hit {
            self.today_hits.fetch_add(1, Ordering::Relaxed);
            self.total_hits.fetch_add(1, Ordering::Relaxed);
        }

        let now_epoch_hour = current_epoch_hour();
        let idx = (now_epoch_hour % 24) as usize;
        let mut slots = self.hourly_slots.write();
        let slot = &mut slots[idx];
        if slot.epoch_hour != now_epoch_hour {
            slot.epoch_hour = now_epoch_hour;
            slot.requests = 0;
            slot.hits = 0;
        }
        slot.requests += 1;
        if is_hit {
            slot.hits += 1;
        }
    }

    pub fn snapshot(&self) -> CacheStatsSnapshot {
        let today_str = Local::now().format("%Y-%m-%d").to_string();
        self.check_rollover(&today_str);

        let t_req = self.today_requests.load(Ordering::Relaxed);
        let t_hit = self.today_hits.load(Ordering::Relaxed);
        let y_req = self.yesterday_requests.load(Ordering::Relaxed);
        let y_hit = self.yesterday_hits.load(Ordering::Relaxed);
        let tot_req = self.total_requests.load(Ordering::Relaxed);
        let tot_hit = self.total_hits.load(Ordering::Relaxed);

        let now_epoch_hour = current_epoch_hour();
        let (r24_req, r24_hit) = {
            let slots = self.hourly_slots.read();
            let mut req_sum = 0u64;
            let mut hit_sum = 0u64;
            for slot in slots.iter() {
                if slot.epoch_hour > 0 && now_epoch_hour.saturating_sub(slot.epoch_hour) < 24 {
                    req_sum += slot.requests;
                    hit_sum += slot.hits;
                }
            }
            (req_sum, hit_sum)
        };

        let make_row = |req: u64, hit: u64| {
            let hit = hit.min(req);
            let misses = req.saturating_sub(hit);
            let hit_rate = if req > 0 {
                (hit as f64 / req as f64) * 100.0
            } else {
                0.0
            };
            CacheStatRow {
                requests: req,
                hits: hit,
                misses,
                hit_rate: (hit_rate * 100.0).round() / 100.0,
            }
        };

        CacheStatsSnapshot {
            yesterday: make_row(y_req, y_hit),
            today: make_row(t_req, t_hit),
            total: make_row(tot_req, tot_hit),
            last_24h: make_row(r24_req, r24_hit),
            updated_at: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }

    pub fn clear(&self) {
        self.today_requests.store(0, Ordering::SeqCst);
        self.today_hits.store(0, Ordering::SeqCst);
        self.yesterday_requests.store(0, Ordering::SeqCst);
        self.yesterday_hits.store(0, Ordering::SeqCst);
        self.total_requests.store(0, Ordering::SeqCst);
        self.total_hits.store(0, Ordering::SeqCst);
        let mut slots = self.hourly_slots.write();
        for slot in slots.iter_mut() {
            *slot = HourlyCacheSlot::default();
        }
    }

    #[allow(dead_code)]
    pub fn load_from_json(&self, json_str: &str) {
        if let Ok(snap) = serde_json::from_str::<CacheStatsSnapshot>(json_str) {
            self.yesterday_requests.store(snap.yesterday.requests, Ordering::SeqCst);
            self.yesterday_hits.store(snap.yesterday.hits, Ordering::SeqCst);
            self.today_requests.store(snap.today.requests, Ordering::SeqCst);
            self.today_hits.store(snap.today.hits, Ordering::SeqCst);
            self.total_requests.store(snap.total.requests, Ordering::SeqCst);
            self.total_hits.store(snap.total.hits, Ordering::SeqCst);
        }
    }

    #[allow(dead_code)]
    pub fn seed(&self, y_req: u64, y_hit: u64, t_req: u64, t_hit: u64, tot_req: u64, tot_hit: u64) {
        self.yesterday_requests.store(y_req, Ordering::SeqCst);
        self.yesterday_hits.store(y_hit, Ordering::SeqCst);
        self.today_requests.store(t_req, Ordering::SeqCst);
        self.today_hits.store(t_hit, Ordering::SeqCst);
        self.total_requests.store(tot_req, Ordering::SeqCst);
        self.total_hits.store(tot_hit, Ordering::SeqCst);
    }
}

fn current_epoch_hour() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        / 3600
}

struct QpsSlot {
    sec: AtomicU64,
    count: AtomicU64,
}

#[derive(Default, Clone)]
struct HourlyStatsSlot {
    epoch_hour: u64,
    hour_label: String,
    // 近 24 小时 KPI 指标分项计数
    total_queries: u64,
    blocked_queries: u64,
    block_eligible_queries: u64,
    cached_queries: u64,
    rewritten_queries: u64,
    cache_eligible_queries: u64,
    total_duration_micros: u64,
    latency_queries: u64,
    // 近 24 小时趋势折线图独立计数
    chart_total: u64,
    chart_blocked: u64,
    chart_duration_micros: u64,
    // 近 24 小时 Top 排行计数
    domain_counts: HashMap<String, u64>,
    blocked_counts: HashMap<String, u64>,
    client_counts: HashMap<String, u64>,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct HourlyMetric {
    pub hour: String,
    pub total: u64,
    pub blocked: u64,
    pub avg_duration_ms: f64,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct MetricsSnapshot {
    pub total_queries: u64,
    pub blocked_queries: u64,
    pub cached_queries: u64,
    pub rewritten_queries: u64,
    pub block_rate: f64,
    pub cache_hit_rate: f64,
    pub avg_duration_ms: f64,
    pub qps: f64,
    pub hourly_history: Vec<HourlyMetric>,
    pub top_domains: Vec<(String, u64)>,
    pub top_blocked: Vec<(String, u64)>,
    pub top_clients: Vec<(String, u64)>,
}

pub struct MetricsCollector {
    slots: RwLock<Vec<HourlyStatsSlot>>,
    qps_slots: [QpsSlot; 5],
    pub cache_stats: Arc<CacheStatsTracker>,
}

impl MetricsCollector {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            slots: RwLock::new((0..24).map(|_| HourlyStatsSlot::default()).collect()),
            qps_slots: [
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
            ],
            cache_stats: CacheStatsTracker::new(),
        })
    }

    pub fn record(&self, ctx: &QueryContext) {
        let micros = (ctx.duration_ms * 1000.0) as u64;
        let is_cached = matches!(ctx.status, QueryStatus::Cached);
        let is_blocked = matches!(ctx.status, QueryStatus::Blocked);
        let is_rewritten = matches!(ctx.status, QueryStatus::Rewritten);
        // 私有域名拒绝、规则拦截与本地重写都在进入缓存查询之前就返回，
        // 它们既不可能命中缓存，也不应稀释缓存命中率。
        let enters_cache_path = !(is_blocked || is_rewritten);
        if enters_cache_path {
            self.cache_stats.record(is_cached);
        }

        // Lock-free atomic QPS slot update
        {
            let now_sec = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let slot = (now_sec % 5) as usize;
            let s = &self.qps_slots[slot];
            if s.sec.load(Ordering::Relaxed) == now_sec {
                s.count.fetch_add(1, Ordering::Relaxed);
            } else {
                s.count.store(1, Ordering::Relaxed);
                s.sec.store(now_sec, Ordering::Relaxed);
            }
        }

        // Record into rolling 24-hour slot
        {
            let now = chrono::Local::now();
            let now_epoch_hour = current_epoch_hour();
            let hour_key = format!("{:02}:00", now.hour());
            let idx = (now_epoch_hour % 24) as usize;

            let mut slots = self.slots.write();
            let slot = &mut slots[idx];
            if slot.epoch_hour != now_epoch_hour {
                *slot = HourlyStatsSlot {
                    epoch_hour: now_epoch_hour,
                    hour_label: hour_key,
                    ..Default::default()
                };
            } else if slot.hour_label.is_empty() {
                slot.hour_label = hour_key;
            }

            slot.total_queries += 1;
            slot.block_eligible_queries += 1;
            slot.total_duration_micros += micros;
            slot.latency_queries += 1;
            if enters_cache_path {
                slot.cache_eligible_queries += 1;
            }
            if is_blocked {
                slot.blocked_queries += 1;
                *slot.blocked_counts.entry(ctx.qname.clone()).or_insert(0) += 1;
                if slot.blocked_counts.len() > 1000 {
                    trim_top_map(&mut slot.blocked_counts, 300);
                }
            } else {
                // 高频查询域名只统计真实解析流量；被拦截的域名单独计入 blocked_counts
                *slot.domain_counts.entry(ctx.qname.clone()).or_insert(0) += 1;
                if slot.domain_counts.len() > 2000 {
                    trim_top_map(&mut slot.domain_counts, 500);
                }
            }
            if is_cached {
                slot.cached_queries += 1;
            }
            if is_rewritten {
                slot.rewritten_queries += 1;
            }

            *slot.client_counts.entry(ctx.client_ip.to_string()).or_insert(0) += 1;
            if slot.client_counts.len() > 500 {
                trim_top_map(&mut slot.client_counts, 200);
            }

            slot.chart_total += 1;
            if is_blocked {
                slot.chart_blocked += 1;
            }
            slot.chart_duration_micros += micros;
        }
    }

    /// 重置指定模块或全部的近 24 小时统计数据
    pub fn reset(&self, scope: &str) {
        let mut slots = self.slots.write();
        match scope {
            "kpi" => {
                for s in slots.iter_mut() {
                    s.total_queries = 0;
                    s.blocked_queries = 0;
                    s.block_eligible_queries = 0;
                    s.cached_queries = 0;
                    s.rewritten_queries = 0;
                    s.cache_eligible_queries = 0;
                    s.total_duration_micros = 0;
                    s.latency_queries = 0;
                }
                for q in &self.qps_slots {
                    q.sec.store(0, Ordering::Relaxed);
                    q.count.store(0, Ordering::Relaxed);
                }
            }
            "kpi_total" => {
                for s in slots.iter_mut() {
                    s.total_queries = 0;
                }
                for q in &self.qps_slots {
                    q.sec.store(0, Ordering::Relaxed);
                    q.count.store(0, Ordering::Relaxed);
                }
            }
            "kpi_blocked" => {
                for s in slots.iter_mut() {
                    s.blocked_queries = 0;
                    s.block_eligible_queries = 0;
                }
            }
            "kpi_cached" => {
                for s in slots.iter_mut() {
                    s.cached_queries = 0;
                    s.cache_eligible_queries = 0;
                }
            }
            "kpi_latency" => {
                for s in slots.iter_mut() {
                    s.total_duration_micros = 0;
                    s.latency_queries = 0;
                }
            }
            "cache_stats" => {
                self.cache_stats.clear();
            }
            "hourly" => {
                for s in slots.iter_mut() {
                    s.chart_total = 0;
                    s.chart_blocked = 0;
                    s.chart_duration_micros = 0;
                }
            }
            "top_domains" => {
                for s in slots.iter_mut() {
                    s.domain_counts.clear();
                }
            }
            "top_blocked" => {
                for s in slots.iter_mut() {
                    s.blocked_counts.clear();
                }
            }
            "top_clients" => {
                for s in slots.iter_mut() {
                    s.client_counts.clear();
                }
            }
            _ => {
                for s in slots.iter_mut() {
                    *s = HourlyStatsSlot::default();
                }
                for q in &self.qps_slots {
                    q.sec.store(0, Ordering::Relaxed);
                    q.count.store(0, Ordering::Relaxed);
                }
                self.cache_stats.clear();
            }
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let now_epoch_hour = current_epoch_hour();
        let now_hour = chrono::Local::now().hour();

        let mut total = 0u64;
        let mut blocked = 0u64;
        let mut block_eligible = 0u64;
        let mut cached = 0u64;
        let mut rewritten = 0u64;
        let mut cache_eligible = 0u64;
        let mut total_dur = 0u64;
        let mut latency_queries = 0u64;

        let mut domain_agg: HashMap<String, u64> = HashMap::new();
        let mut blocked_agg: HashMap<String, u64> = HashMap::new();
        let mut client_agg: HashMap<String, u64> = HashMap::new();

        let slots = self.slots.read();
        for slot in slots.iter() {
            if slot.epoch_hour > 0 && now_epoch_hour.saturating_sub(slot.epoch_hour) < 24 {
                total += slot.total_queries;
                blocked += slot.blocked_queries;
                block_eligible += slot.block_eligible_queries;
                cached += slot.cached_queries;
                rewritten += slot.rewritten_queries;
                cache_eligible += slot.cache_eligible_queries;
                total_dur += slot.total_duration_micros;
                latency_queries += slot.latency_queries;

                for (k, v) in &slot.domain_counts {
                    *domain_agg.entry(k.clone()).or_insert(0) += *v;
                }
                for (k, v) in &slot.blocked_counts {
                    *blocked_agg.entry(k.clone()).or_insert(0) += *v;
                }
                for (k, v) in &slot.client_counts {
                    *client_agg.entry(k.clone()).or_insert(0) += *v;
                }
            }
        }

        let avg_dur = if latency_queries > 0 {
            (total_dur as f64 / latency_queries as f64) / 1000.0
        } else {
            0.0
        };

        let block_rate = if block_eligible > 0 {
            ((blocked.min(block_eligible)) as f64 / block_eligible as f64) * 100.0
        } else {
            0.0
        };

        let cache_hit_rate = if cache_eligible > 0 {
            ((cached.min(cache_eligible)) as f64 / cache_eligible as f64) * 100.0
        } else {
            0.0
        };

        let qps = {
            let now_sec = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let mut sum = 0u64;
            for slot in &self.qps_slots {
                let sec = slot.sec.load(Ordering::Relaxed);
                if now_sec >= sec && now_sec - sec < 5 {
                    sum += slot.count.load(Ordering::Relaxed);
                }
            }
            sum as f64 / 5.0
        };

        let top_domains = {
            let mut list: Vec<(String, u64)> = domain_agg.into_iter().collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(50);
            list
        };

        let top_blocked = {
            let mut list: Vec<(String, u64)> = blocked_agg.into_iter().collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(50);
            list
        };

        let top_clients = {
            let mut list: Vec<(String, u64)> = client_agg.into_iter().collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(50);
            list
        };

        let hourly_history = {
            let mut res = Vec::with_capacity(24);
            for i in 0..24 {
                let offset = 23 - i;
                let h = (now_hour + 24 - offset) % 24;
                let hour_key = format!("{:02}:00", h);
                let target_epoch_hour = now_epoch_hour.saturating_sub(offset as u64);
                let slot = &slots[(target_epoch_hour % 24) as usize];
                if slot.epoch_hour == target_epoch_hour && slot.chart_total > 0 {
                    let avg = (slot.chart_duration_micros as f64 / slot.chart_total as f64) / 1000.0;
                    res.push(HourlyMetric {
                        hour: hour_key,
                        total: slot.chart_total,
                        blocked: slot.chart_blocked,
                        avg_duration_ms: avg,
                    });
                } else {
                    res.push(HourlyMetric {
                        hour: hour_key,
                        total: 0,
                        blocked: 0,
                        avg_duration_ms: 0.0,
                    });
                }
            }
            res
        };

        MetricsSnapshot {
            total_queries: total,
            blocked_queries: blocked,
            cached_queries: cached,
            rewritten_queries: rewritten,
            block_rate,
            cache_hit_rate,
            avg_duration_ms: avg_dur,
            qps,
            hourly_history,
            top_domains,
            top_blocked,
            top_clients,
        }
    }
}

fn trim_top_map(map: &mut HashMap<String, u64>, keep: usize) {
    let mut list: Vec<(String, u64)> = map.drain().collect();
    list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
    for (k, v) in list.into_iter().take(keep) {
        map.insert(k, v);
    }
}
