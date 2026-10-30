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
    pub updated_at: String,
}

pub struct CacheStatsTracker {
    current_date: RwLock<String>,
    today_requests: AtomicU64,
    today_hits: AtomicU64,
    yesterday_requests: AtomicU64,
    yesterday_hits: AtomicU64,
    total_requests: AtomicU64,
    total_hits: AtomicU64,
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
    }

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

struct QpsSlot {
    sec: AtomicU64,
    count: AtomicU64,
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
    total_queries: AtomicU64,
    blocked_queries: AtomicU64,
    cached_queries: AtomicU64,
    rewritten_queries: AtomicU64,
    total_duration_micros: AtomicU64,
    domain_counts: Arc<RwLock<HashMap<String, u64>>>,
    blocked_counts: Arc<RwLock<HashMap<String, u64>>>,
    client_counts: Arc<RwLock<HashMap<String, u64>>>,
    qps_slots: [QpsSlot; 5],
    hourly_buckets: Arc<RwLock<HashMap<String, (u64, u64, u64)>>>, // hour_key -> (total, blocked, total_micros)
    pub cache_stats: Arc<CacheStatsTracker>,
}

impl MetricsCollector {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            total_queries: AtomicU64::new(0),
            blocked_queries: AtomicU64::new(0),
            cached_queries: AtomicU64::new(0),
            rewritten_queries: AtomicU64::new(0),
            total_duration_micros: AtomicU64::new(0),
            domain_counts: Arc::new(RwLock::new(HashMap::new())),
            blocked_counts: Arc::new(RwLock::new(HashMap::new())),
            client_counts: Arc::new(RwLock::new(HashMap::new())),
            qps_slots: [
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
                QpsSlot { sec: AtomicU64::new(0), count: AtomicU64::new(0) },
            ],
            hourly_buckets: Arc::new(RwLock::new(HashMap::new())),
            cache_stats: CacheStatsTracker::new(),
        })
    }

    pub fn record(&self, ctx: &QueryContext) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        let micros = (ctx.duration_ms * 1000.0) as u64;
        self.total_duration_micros.fetch_add(micros, Ordering::Relaxed);
        let is_cached = matches!(ctx.status, QueryStatus::Cached);
        self.cache_stats.record(is_cached);

        match ctx.status {
            QueryStatus::Blocked => {
                self.blocked_queries.fetch_add(1, Ordering::Relaxed);
                let mut w = self.blocked_counts.write();
                *w.entry(ctx.qname.clone()).or_insert(0) += 1;
            }
            QueryStatus::Cached => {
                self.cached_queries.fetch_add(1, Ordering::Relaxed);
            }
            QueryStatus::Rewritten => {
                self.rewritten_queries.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }

        let sample = if is_cached {
            (self.cached_queries.load(Ordering::Relaxed) % 10) == 0
        } else {
            true
        };

        if sample {
            let inc = if is_cached { 10 } else { 1 };
            let mut w = self.domain_counts.write();
            *w.entry(ctx.qname.clone()).or_insert(0) += inc;
        }

        if sample {
            let inc = if is_cached { 10 } else { 1 };
            let mut w = self.client_counts.write();
            *w.entry(ctx.client_ip.to_string()).or_insert(0) += inc;
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

        // Record hourly bucket
        {
            let now = chrono::Local::now();
            let hour_key = format!("{:02}:00", now.hour());
            let is_blocked = matches!(ctx.status, QueryStatus::Blocked);
            let mut w = self.hourly_buckets.write();
            let entry = w.entry(hour_key).or_insert((0, 0, 0));
            entry.0 += 1;
            if is_blocked {
                entry.1 += 1;
            }
            entry.2 += micros;
        }
    }

    pub fn snapshot(&self) -> MetricsSnapshot {
        let total = self.total_queries.load(Ordering::Relaxed);
        let blocked = self.blocked_queries.load(Ordering::Relaxed);
        let cached = self.cached_queries.load(Ordering::Relaxed);
        let rewritten = self.rewritten_queries.load(Ordering::Relaxed);
        let total_dur = self.total_duration_micros.load(Ordering::Relaxed);

        let avg_dur = if total > 0 {
            (total_dur as f64 / total as f64) / 1000.0
        } else {
            0.0
        };

        let block_rate = if total > 0 {
            (blocked as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        let cache_hit_rate = if total > 0 {
            (cached as f64 / total as f64) * 100.0
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
            let r = self.domain_counts.read();
            let mut list: Vec<(String, u64)> = r.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(50);
            list
        };

        let top_blocked = {
            let r = self.blocked_counts.read();
            let mut list: Vec<(String, u64)> = r.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(50);
            list
        };

        let top_clients = {
            let r = self.client_counts.read();
            let mut list: Vec<(String, u64)> = r.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(50);
            list
        };

        let hourly_history = {
            let r = self.hourly_buckets.read();
            let now_hour = chrono::Local::now().hour();
            let mut res = Vec::with_capacity(24);
            for i in 0..24 {
                let h = (now_hour + 24 - (23 - i)) % 24;
                let hour_key = format!("{:02}:00", h);
                if let Some(&(tot, blk, dur)) = r.get(&hour_key) {
                    let avg = if tot > 0 { (dur as f64 / tot as f64) / 1000.0 } else { 0.0 };
                    res.push(HourlyMetric {
                        hour: hour_key,
                        total: tot,
                        blocked: blk,
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

    pub fn get_hourly_buckets_json(&self) -> String {
        let r = self.hourly_buckets.read();
        serde_json::to_string(&*r).unwrap_or_default()
    }

    pub fn load_hourly_buckets_json(&self, json_str: &str) {
        if let Ok(map) = serde_json::from_str::<HashMap<String, (u64, u64, u64)>>(json_str) {
            *self.hourly_buckets.write() = map;
        }
    }
}
