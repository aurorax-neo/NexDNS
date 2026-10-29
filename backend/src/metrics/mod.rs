use chrono::{Timelike, Utc};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use crate::pipeline::context::{QueryContext, QueryStatus};

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
    recent_qps_window: Arc<RwLock<Vec<(std::time::Instant, u64)>>>,
    hourly_buckets: Arc<RwLock<HashMap<String, (u64, u64, u64)>>>, // hour_key -> (total, blocked, total_micros)
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
            recent_qps_window: Arc::new(RwLock::new(Vec::new())),
            hourly_buckets: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub fn record(&self, ctx: &QueryContext) {
        self.total_queries.fetch_add(1, Ordering::Relaxed);
        let micros = (ctx.duration_ms * 1000.0) as u64;
        self.total_duration_micros.fetch_add(micros, Ordering::Relaxed);

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

        {
            let mut w = self.domain_counts.write();
            *w.entry(ctx.qname.clone()).or_insert(0) += 1;
        }

        {
            let mut w = self.client_counts.write();
            *w.entry(ctx.client_ip.to_string()).or_insert(0) += 1;
        }

        {
            let now = std::time::Instant::now();
            let mut w = self.recent_qps_window.write();
            w.push((now, 1));
            w.retain(|(t, _)| t.elapsed().as_secs() <= 5);
        }

        // Record hourly bucket
        {
            let now = Utc::now();
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
            let w = self.recent_qps_window.read();
            if w.is_empty() {
                0.0
            } else {
                let count: u64 = w.iter().map(|(_, c)| *c).sum();
                count as f64 / 5.0
            }
        };

        let top_domains = {
            let r = self.domain_counts.read();
            let mut list: Vec<(String, u64)> = r.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(10);
            list
        };

        let top_blocked = {
            let r = self.blocked_counts.read();
            let mut list: Vec<(String, u64)> = r.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(10);
            list
        };

        let top_clients = {
            let r = self.client_counts.read();
            let mut list: Vec<(String, u64)> = r.iter().map(|(k, v)| (k.clone(), *v)).collect();
            list.sort_by_key(|(_, v)| std::cmp::Reverse(*v));
            list.truncate(10);
            list
        };

        let hourly_history = {
            let r = self.hourly_buckets.read();
            let now_hour = Utc::now().hour();
            let mut res = Vec::new();
            for i in 0..24 {
                let h = (now_hour + 25 - i) % 24;
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
            res.reverse();
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
