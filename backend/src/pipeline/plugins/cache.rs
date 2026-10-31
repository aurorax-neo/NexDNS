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
    pub prefetched: Arc<std::sync::atomic::AtomicBool>,
}

pub struct CachePlugin {
    entries: Arc<RwLock<HashMap<(String, RecordType), CacheEntry>>>,
    max_size: usize,
    pub prefetch: std::sync::atomic::AtomicBool,
    pub min_ttl: AtomicU32,
    pub max_ttl: AtomicU32,
}

impl CachePlugin {
    pub fn new(max_size: usize, prefetch: bool, min_ttl: u32, max_ttl: u32) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::with_capacity(max_size))),
            max_size,
            prefetch: std::sync::atomic::AtomicBool::new(prefetch),
            min_ttl: AtomicU32::new(min_ttl),
            max_ttl: AtomicU32::new(max_ttl),
        }
    }

    pub fn get_with_prefetch(&self, qname: &str, qtype: RecordType) -> Option<(Message, bool)> {
        let r = self.entries.read();
        let key = (qname.to_lowercase(), qtype);
        if let Some(entry) = r.get(&key) {
            entry.hits.fetch_add(1, Ordering::Relaxed);
            let elapsed = entry.created_at.elapsed();
            // 严格遵循 TTL：未过期极速秒返，过期返回 None 触发正常回源放行
            if elapsed <= entry.ttl {
                let mut resp = entry.message.clone();
                let remaining_ttl = (entry.ttl - elapsed).as_secs().max(1) as u32;
                for ans in &mut resp.answers {
                    ans.ttl = remaining_ttl;
                }
                // 剩余 TTL 低于 20% 时 (即消耗超过 80%)，若开启 prefetch 且尚未预取过，触发异步预取
                let should_prefetch = self.prefetch.load(Ordering::Relaxed)
                    && (elapsed * 5 >= entry.ttl * 4)
                    && entry.prefetched.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed).is_ok();
                return Some((resp, should_prefetch));
            }
        }
        None
    }

    pub fn get(&self, qname: &str, qtype: RecordType) -> Option<Message> {
        self.get_with_prefetch(qname, qtype).map(|(msg, _)| msg)
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

        let key = (qname.to_lowercase(), qtype);
        let mut w = self.entries.write();
        if !w.contains_key(&key) && w.len() >= self.max_size {
            // 智能非阻塞微秒级采样驱逐策略 (类 Redis 采样法):
            // 避免在持有写锁时全量排序 10,000 条条目，仅抽样 64 条候选进行极速驱逐，消除微突发抖动
            let evict_count = (self.max_size / 50).clamp(1, 20);
            let mut sample: Vec<((String, RecordType), bool, u64, Instant)> = w
                .iter()
                .take(64)
                .map(|(k, v)| {
                    let is_expired = v.created_at.elapsed() > v.ttl;
                    let hits = v.hits.load(Ordering::Relaxed);
                    (k.clone(), is_expired, hits, v.created_at)
                })
                .collect();

            sample.sort_by(|a, b| {
                b.1.cmp(&a.1) // 过期项优先驱逐
                    .then_with(|| a.2.cmp(&b.2)) // 命中数低者优先
                    .then_with(|| a.3.cmp(&b.3)) // 创建时间早者优先
            });

            for (k, _, _, _) in sample.into_iter().take(evict_count) {
                w.remove(&k);
            }
        }

        w.insert(
            key,
            CacheEntry {
                message,
                created_at: Instant::now(),
                ttl: Duration::from_secs(effective_ttl as u64),
                hits: Arc::new(AtomicU64::new(0)),
                prefetched: Arc::new(std::sync::atomic::AtomicBool::new(false)),
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
        if let Some(mut cached_resp) = self.get(&ctx.qname, ctx.qtype) {
            cached_resp.metadata.id = ctx.request.metadata.id;
            cached_resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            cached_resp.metadata.recursion_available = true;

            ctx.response = Some(cached_resp);
            ctx.status = QueryStatus::Cached;
            ctx.tags.insert("hit_cache".to_string());
            return PluginResult::Accept;
        }

        PluginResult::Continue
    }
}
