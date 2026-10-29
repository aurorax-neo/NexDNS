use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use parking_lot::RwLock;
use hickory_proto::op::Message;
use hickory_proto::rr::RecordType;
use crate::pipeline::context::{QueryContext, QueryStatus};
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

#[derive(Clone)]
pub struct CacheEntry {
    pub message: Message,
    pub created_at: Instant,
    pub ttl: Duration,
    pub hits: u64,
}

pub struct CachePlugin {
    entries: Arc<RwLock<HashMap<(String, RecordType), CacheEntry>>>,
    max_size: usize,
    serve_expired: bool,
    prefetch: bool,
    pub min_ttl: AtomicU32,
    pub max_ttl: AtomicU32,
}

impl CachePlugin {
    pub fn new(max_size: usize, serve_expired: bool, prefetch: bool, min_ttl: u32, max_ttl: u32) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::with_capacity(max_size))),
            max_size,
            serve_expired,
            prefetch,
            min_ttl: AtomicU32::new(min_ttl),
            max_ttl: AtomicU32::new(max_ttl),
        }
    }

    pub fn get(&self, qname: &str, qtype: RecordType) -> Option<(Message, bool)> {
        let mut w = self.entries.write();
        let key = (qname.to_lowercase(), qtype);
        if let Some(entry) = w.get_mut(&key) {
            entry.hits += 1;
            let elapsed = entry.created_at.elapsed();
            if elapsed <= entry.ttl {
                let mut resp = entry.message.clone();
                let remaining_ttl = (entry.ttl - elapsed).as_secs().max(1) as u32;
                for ans in &mut resp.answers {
                    ans.ttl = remaining_ttl;
                }
                return Some((resp, false));
            } else if self.serve_expired {
                let mut resp = entry.message.clone();
                for ans in &mut resp.answers {
                    ans.ttl = 3;
                }
                return Some((resp, true));
            }
        }
        None
    }

    pub fn insert(&self, qname: &str, qtype: RecordType, message: Message) {
        let min_limit = self.min_ttl.load(Ordering::Relaxed).max(1);
        let max_limit = self.max_ttl.load(Ordering::Relaxed).max(min_limit);

        let mut record_ttl = 300u32;
        if !message.answers.is_empty() {
            let mut found = u32::MAX;
            for ans in &message.answers {
                found = found.min(ans.ttl);
            }
            if found != u32::MAX {
                record_ttl = found;
            }
        }
        let effective_ttl = record_ttl.clamp(min_limit, max_limit);

        let mut w = self.entries.write();
        if w.len() >= self.max_size {
            // Evict oldest or excess
            let keys_to_remove: Vec<_> = w.keys().take(100).cloned().collect();
            for k in keys_to_remove {
                w.remove(&k);
            }
        }

        let key = (qname.to_lowercase(), qtype);
        w.insert(
            key,
            CacheEntry {
                message,
                created_at: Instant::now(),
                ttl: Duration::from_secs(effective_ttl as u64),
                hits: 0,
            },
        );
    }

    pub fn flush(&self) {
        let mut w = self.entries.write();
        w.clear();
    }

    pub fn size(&self) -> usize {
        self.entries.read().len()
    }
}

#[async_trait]
impl DnsPlugin for CachePlugin {
    fn name(&self) -> &str {
        "cache"
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        if let Some((mut cached_resp, is_expired)) = self.get(&ctx.qname, ctx.qtype) {
            cached_resp.metadata.id = ctx.request.metadata.id;
            cached_resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            cached_resp.metadata.recursion_available = true;

            ctx.response = Some(cached_resp);
            ctx.status = QueryStatus::Cached;
            if is_expired {
                ctx.tags.insert("hit_cache_expired".to_string());
            } else {
                ctx.tags.insert("hit_cache".to_string());
            }
            return PluginResult::Accept;
        }

        PluginResult::Continue
    }
}
