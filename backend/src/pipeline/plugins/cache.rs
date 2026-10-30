use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
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
    pub hits: Arc<AtomicU64>,
}

pub struct CachePlugin {
    entries: Arc<RwLock<HashMap<(String, RecordType), CacheEntry>>>,
    max_size: usize,
    serve_expired: bool,
    #[allow(dead_code)]
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
        let r = self.entries.read();
        let key = (qname.to_lowercase(), qtype);
        if let Some(entry) = r.get(&key) {
            entry.hits.fetch_add(1, Ordering::Relaxed);
            let elapsed = entry.created_at.elapsed();
            if elapsed <= entry.ttl {
                let mut resp = entry.message.clone();
                let remaining_ttl = (entry.ttl - elapsed).as_secs().max(1) as u32;
                for ans in &mut resp.answers {
                    ans.ttl = remaining_ttl;
                }
                return Some((resp, false));
            } else if self.serve_expired {
                // 过期记录在 1 小时内允许通过 serve-expired 闪速返回，同时触发后台静默拉取更新；超过 1 小时强制同步回源
                if elapsed <= Duration::from_secs(3600) {
                    let mut resp = entry.message.clone();
                    for ans in &mut resp.answers {
                        ans.ttl = 3;
                    }
                    return Some((resp, true));
                }
            }
        }
        None
    }

    pub fn insert(&self, qname: &str, qtype: RecordType, mut message: Message) {
        // 根据 RFC 6891 / RFC 7873，EDNS(0) 与 Cookie 属于逐跳 (Hop-by-hop) 协议头，严禁存入 DNS 共享缓存中
        message.edns = None;

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
            // 智能驱逐策略: 优先清理已过期记录，其次清理低频命中(LFU)与最老记录(LRU)
            let mut candidates: Vec<((String, RecordType), bool, u64, Instant)> = w
                .iter()
                .map(|(k, v)| {
                    let is_expired = v.created_at.elapsed() > v.ttl;
                    let hits = v.hits.load(Ordering::Relaxed);
                    (k.clone(), is_expired, hits, v.created_at)
                })
                .collect();

            candidates.sort_by(|a, b| {
                b.1.cmp(&a.1) // 过期项优先驱逐
                    .then_with(|| a.2.cmp(&b.2)) // 命中数低者优先
                    .then_with(|| a.3.cmp(&b.3)) // 创建时间早者优先
            });

            let evict_count = (self.max_size / 20).clamp(1, 100).min(candidates.len());
            for (k, _, _, _) in candidates.into_iter().take(evict_count) {
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
                hits: Arc::new(AtomicU64::new(0)),
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
