use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::{RData, RecordType};
use crate::pipeline::context::{QueryContext, QueryStatus};
use crate::pipeline::plugin::DnsPlugin;
use crate::pipeline::plugins::adblock::AdblockPlugin;
use crate::pipeline::plugins::cache::CachePlugin;
use crate::pipeline::plugins::domain_match::{DomainListRegistry, DomainMatcher};
use crate::pipeline::plugins::ip_match::IpListRegistry;
use crate::pipeline::plugins::rewrite::RewritePlugin;
use crate::pipeline::plugins::speed_check::SpeedCheckPlugin;
use crate::pipeline::plugins::upstream::ForwardPlugin;

#[derive(Clone)]
struct InflightSnapshot {
    response: Option<Message>,
    status: QueryStatus,
    routing_group: Option<String>,
    fastest_ip: Option<IpAddr>,
    tags: HashSet<String>,
}

type InflightMap = parking_lot::Mutex<HashMap<(String, RecordType), watch::Sender<Option<InflightSnapshot>>>>;

struct InflightGuard {
    map: Arc<InflightMap>,
    key: (String, RecordType),
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.map.lock().remove(&self.key);
    }
}

#[derive(Clone)]
pub struct PipelineEngine {
    pub rewrite: Arc<RewritePlugin>,
    pub adblock: Arc<AdblockPlugin>,
    pub cache: Arc<CachePlugin>,
    pub domain_registry: Arc<DomainListRegistry>,
    pub ip_registry: Arc<IpListRegistry>,
    pub forward_mainland: Arc<ForwardPlugin>,
    pub forward_gfw: Arc<ForwardPlugin>,
    pub forward_overseas: Arc<ForwardPlugin>,
    pub speed_check: Arc<SpeedCheckPlugin>,
    pub adblock_enabled: Arc<AtomicBool>,
    pub cache_enabled: Arc<AtomicBool>,
    pub router_enabled: Arc<AtomicBool>,
    pub anti_pollution_enabled: Arc<AtomicBool>,
    pub speed_check_enabled: Arc<AtomicBool>,
    pub fallback_timeout_ms: Arc<AtomicU64>,
    pub response_mode: Arc<parking_lot::RwLock<String>>,
    inflight: Arc<InflightMap>,
}

impl PipelineEngine {
    pub fn new(
        rewrite: Arc<RewritePlugin>,
        adblock: Arc<AdblockPlugin>,
        cache: Arc<CachePlugin>,
        domain_registry: Arc<DomainListRegistry>,
        ip_registry: Arc<IpListRegistry>,
        forward_mainland: Arc<ForwardPlugin>,
        forward_gfw: Arc<ForwardPlugin>,
        forward_overseas: Arc<ForwardPlugin>,
        speed_check: Arc<SpeedCheckPlugin>,
        router_enabled: bool,
        anti_pollution_enabled: bool,
        response_mode: String,
    ) -> Self {
        let shared_speed_enabled = speed_check.enabled.clone();
        Self {
            rewrite,
            adblock,
            cache,
            domain_registry,
            ip_registry,
            forward_mainland,
            forward_gfw,
            forward_overseas,
            speed_check,
            adblock_enabled: Arc::new(AtomicBool::new(true)),
            cache_enabled: Arc::new(AtomicBool::new(true)),
            router_enabled: Arc::new(AtomicBool::new(router_enabled)),
            anti_pollution_enabled: Arc::new(AtomicBool::new(anti_pollution_enabled)),
            speed_check_enabled: shared_speed_enabled,
            fallback_timeout_ms: Arc::new(AtomicU64::new(1500)),
            response_mode: Arc::new(parking_lot::RwLock::new(response_mode)),
            inflight: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        }
    }

    pub fn set_fallback_timeout_ms(&self, timeout_ms: u64) {
        self.fallback_timeout_ms.store(timeout_ms.clamp(100, 30000), Ordering::SeqCst);
    }

    pub fn update_custom_routing_domains(&self, custom_direct: &[String], custom_proxy: &[String]) {
        let mut direct_matcher = DomainMatcher::new("custom_direct");
        if !custom_direct.is_empty() {
            direct_matcher.load_from_text(&custom_direct.join("\n"));
        }
        self.domain_registry.register(direct_matcher);

        let mut proxy_matcher = DomainMatcher::new("custom_proxy");
        if !custom_proxy.is_empty() {
            proxy_matcher.load_from_text(&custom_proxy.join("\n"));
        }
        self.domain_registry.register(proxy_matcher);
    }

    pub fn toggle_stage(&self, stage_id: &str, enabled: bool) -> bool {
        match stage_id {
            "adblock" => {
                self.adblock_enabled.store(enabled, Ordering::SeqCst);
                true
            }
            "cache" => {
                self.cache_enabled.store(enabled, Ordering::SeqCst);
                true
            }
            "router" => {
                if enabled {
                    let gfw_empty = self.forward_gfw.group.upstreams.read().is_empty();
                    let overseas_empty = self.forward_overseas.group.upstreams.read().is_empty();
                    if gfw_empty || overseas_empty {
                        return false;
                    }
                }
                self.router_enabled.store(enabled, Ordering::SeqCst);
                true
            }
            "anti_pollution" => {
                self.anti_pollution_enabled.store(enabled, Ordering::SeqCst);
                true
            }
            "speed_check" => {
                // speed_check_enabled 与 speed_check.enabled 共享同一 Arc<AtomicBool>
                self.speed_check.set_enabled(enabled);
                true
            }
            _ => false,
        }
    }

    pub fn reload_upstreams(
        &self,
        mainland: Vec<crate::pipeline::plugins::upstream::UpstreamServer>,
        gfw: Vec<crate::pipeline::plugins::upstream::UpstreamServer>,
        overseas: Vec<crate::pipeline::plugins::upstream::UpstreamServer>,
    ) {
        self.forward_mainland.group.set_servers(mainland);
        self.forward_gfw.group.set_servers(gfw);
        self.forward_overseas.group.set_servers(overseas);
    }

    pub fn set_upstream_timeout_ms(&self, timeout_ms: u64) {
        self.forward_mainland.group.set_timeout_ms(timeout_ms);
        self.forward_gfw.group.set_timeout_ms(timeout_ms);
        self.forward_overseas.group.set_timeout_ms(timeout_ms);
    }

    pub fn has_china_ip(&self, resp: &Message) -> bool {
        for ans in &resp.answers {
            match &ans.data {
                RData::A(a) => {
                    let ip = IpAddr::V4(a.0);
                    if self.ip_registry.contains("ipv4_china", &ip) {
                        return true;
                    }
                }
                RData::AAAA(aaaa) => {
                    let ip = IpAddr::V6(aaaa.0);
                    if aaaa.0 == std::net::Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 1)
                        || aaaa.0 == std::net::Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 2)
                    {
                        return false;
                    }
                    if self.ip_registry.contains("ipv6_china", &ip) {
                        return true;
                    }
                }
                _ => {}
            }
        }
        false
    }

    pub fn has_ip_answers(&self, resp: &Message) -> bool {
        for ans in &resp.answers {
            if matches!(&ans.data, RData::A(_) | RData::AAAA(_)) {
                return true;
            }
        }
        false
    }

    fn apply_response_mode(&self, ctx: &mut QueryContext) {
        let mode = self.response_mode.read().clone();
        if let Some(ref mut resp) = ctx.response {
            match mode.as_str() {
                "ipv4" | "ipv4_only" => {
                    if ctx.qtype == RecordType::AAAA {
                        resp.answers.clear();
                    } else {
                        resp.answers.retain(|ans| !matches!(ans.data, RData::AAAA(_)));
                    }
                }
                "ipv6" | "ipv6_only" => {
                    if ctx.qtype == RecordType::A {
                        resp.answers.clear();
                    } else {
                        resp.answers.retain(|ans| !matches!(ans.data, RData::A(_)));
                    }
                }
                _ => {}
            }
        }
    }

    async fn query_proxy_chain(&self, ctx: &mut QueryContext) {
        self.forward_gfw.execute(ctx).await;

        let gfw_ok = ctx
            .response
            .as_ref()
            .map(|r| r.metadata.response_code == ResponseCode::NoError && !r.answers.is_empty())
            .unwrap_or(false);

        if !gfw_ok {
            self.forward_overseas.execute(ctx).await;
        }

        let overseas_ok = ctx
            .response
            .as_ref()
            .map(|r| r.metadata.response_code == ResponseCode::NoError && !r.answers.is_empty())
            .unwrap_or(false);

        // 容灾兜底：若代理未开启或境外上游不可达，自动降级至大陆上游解析，防止 proxy 域名直接返回 ServFail 断网
        if !overseas_ok {
            ctx.tags.insert("fallback_restore_mainland".to_string());
            self.forward_mainland.execute(ctx).await;
        }
    }

    async fn resolve_uncached_and_store(&self, ctx: &mut QueryContext, is_prefetch: bool) {
        let qname_lower = ctx.qname.to_ascii_lowercase();
        ctx.record_timing("route");

        if self.domain_registry.matches("domain_private", &ctx.qname)
            || is_rfc_locally_served_domain(&qname_lower)
        {
            let is_public_direct_helper = qname_lower.ends_with(".qq.com")
                || qname_lower.ends_with(".plex.direct")
                || qname_lower.ends_with(".ts.net")
                || qname_lower.ends_with(".kaspersky-labs.com")
                || qname_lower.ends_with(".adguard.org")
                || qname_lower.ends_with(".arubanetworks.com")
                || qname_lower.ends_with(".h3c.com");

            if is_public_direct_helper {
                ctx.tags.insert("route:private_direct".to_string());
                self.forward_mainland.execute(ctx).await;
            } else {
                let mut resp = Message::error_msg(
                    ctx.request.metadata.id,
                    ctx.request.metadata.op_code,
                    ResponseCode::NXDomain,
                );
                resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                resp.metadata.recursion_available = true;
                resp.queries = ctx.request.queries.clone();
                attach_negative_soa(&mut resp, &qname_lower, 300);
                if self.cache_enabled.load(Ordering::Relaxed) {
                    self.cache.insert(&ctx.qname, ctx.qtype, resp.clone());
                }
                ctx.response = Some(resp);
                ctx.status = QueryStatus::Allowed;
                ctx.routing_group = Some("private_local".to_string());
                return;
            }
        } else if !self.router_enabled.load(Ordering::Relaxed) {
            ctx.tags.insert("route:bypass_disabled".to_string());
            self.forward_mainland.execute(ctx).await;
        } else {
            // 最高优先级：用户自定义持久化分流覆盖表（高于 direct.txt / proxy.txt，免受在线自动更新覆盖）
            let custom_direct_spec = self.domain_registry.match_specificity("custom_direct", &ctx.qname);
            let custom_proxy_spec = self.domain_registry.match_specificity("custom_proxy", &ctx.qname);

            if custom_proxy_spec.is_some() || custom_direct_spec.is_some() {
                if custom_proxy_spec >= custom_direct_spec {
                    ctx.tags.insert("route:custom_proxy".to_string());
                    ctx.matched_rule = Some("custom_proxy_domains".to_string());
                    self.query_proxy_chain(ctx).await;
                } else {
                    ctx.tags.insert("route:custom_direct".to_string());
                    ctx.matched_rule = Some("custom_direct_domains".to_string());
                    self.forward_mainland.execute(ctx).await;
                }
            } else {
                let is_direct = self.domain_registry.matches("domain_direct", &ctx.qname);
                let is_proxy = self.domain_registry.matches("domain_proxy", &ctx.qname);

                if is_direct {
                    ctx.tags.insert("route:direct".to_string());
                    self.forward_mainland.execute(ctx).await;
                } else if is_proxy {
                    ctx.tags.insert("route:proxy".to_string());
                    self.query_proxy_chain(ctx).await;
                } else {
                    // Fallback pipeline: try mainland first (bounded by fallback_timeout_ms)
                    ctx.tags.insert("route:fallback".to_string());
                    let fb_timeout = Duration::from_millis(self.fallback_timeout_ms.load(Ordering::Relaxed));
                    self.forward_mainland
                        .execute_with_timeout(ctx, Some(fb_timeout))
                        .await;

                    let mut mainland_fallback_resp = None;

                    // Anti-pollution check: if mainland returned an IP that is NOT in China IP list, drop response and fallback to GFW!
                    if self.anti_pollution_enabled.load(Ordering::Relaxed) {
                        if let Some(ref resp) = ctx.response {
                            if self.has_ip_answers(resp) && !self.has_china_ip(resp) {
                                ctx.tags.insert("pollution_detected".to_string());
                                mainland_fallback_resp = ctx.response.take();
                            }
                        }
                    }

                    // If mainland returned no response or dropped due to pollution, query GFW/Overseas
                    if ctx.response.is_none() {
                        ctx.tags.insert("fallback_to_gfw".to_string());
                        self.forward_gfw.execute(ctx).await;

                        let gfw_effective = match &ctx.response {
                            Some(r) => r.metadata.response_code == ResponseCode::NoError && !r.answers.is_empty(),
                            None => false,
                        };

                        if !gfw_effective {
                            self.forward_overseas.execute(ctx).await;
                        }

                        let overseas_effective = match &ctx.response {
                            Some(r) => r.metadata.response_code == ResponseCode::NoError && !r.answers.is_empty(),
                            None => false,
                        };

                        if !overseas_effective && mainland_fallback_resp.is_some() {
                            ctx.tags.insert("fallback_restore_mainland".to_string());
                            ctx.response = mainland_fallback_resp;
                            ctx.routing_group = Some("mainland (fallback)".to_string());
                        }
                    }
                }
            }
        }

        // Post-processing: Response Mode -> Speed Check -> Cache Storage
        if ctx.response.is_some() {
            self.apply_response_mode(ctx);

            if self.speed_check_enabled.load(Ordering::Relaxed) {
                ctx.record_timing("speed_check");
                self.speed_check.execute(ctx).await;
            }

            if self.cache_enabled.load(Ordering::Relaxed) {
                if let Some(ref resp) = ctx.response {
                    let degraded_polluted_fallback = ctx.tags.contains("pollution_detected")
                        && ctx.tags.contains("fallback_restore_mainland");
                    let should_store = if is_prefetch {
                        // 后台预取严禁将境外短暂断连时降级回退的污染 IP 覆盖已有干净缓存
                        resp.metadata.response_code == ResponseCode::NoError
                            && !resp.answers.is_empty()
                            && !degraded_polluted_fallback
                    } else {
                        resp.metadata.response_code == ResponseCode::NoError
                            || resp.metadata.response_code == ResponseCode::NXDomain
                    };
                    if should_store {
                        self.cache.insert(&ctx.qname, ctx.qtype, resp.clone());
                    }
                }
            }
        } else if !is_prefetch {
            let mut resp = Message::error_msg(
                ctx.request.metadata.id,
                ctx.request.metadata.op_code,
                ResponseCode::ServFail,
            );
            resp.queries = ctx.request.queries.clone();
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Error;
        }
    }

    /// Singleflight 在途查询合并去重：同一时刻针对相同 (qname, qtype) 的并发冷查询共享单次上游解析
    async fn resolve_with_singleflight(&self, ctx: &mut QueryContext, is_prefetch: bool) {
        enum FlightRole {
            Leader(watch::Sender<Option<InflightSnapshot>>, InflightGuard),
            Follower(watch::Receiver<Option<InflightSnapshot>>),
        }

        let key = (ctx.qname.to_ascii_lowercase(), ctx.qtype);
        let role = {
            let mut map = self.inflight.lock();
            if let Some(existing_tx) = map.get(&key) {
                FlightRole::Follower(existing_tx.subscribe())
            } else {
                let (tx, _rx) = watch::channel(None);
                map.insert(key.clone(), tx.clone());
                FlightRole::Leader(
                    tx,
                    InflightGuard {
                        map: self.inflight.clone(),
                        key,
                    },
                )
            }
        };

        match role {
            FlightRole::Leader(tx, _guard) => {
                self.resolve_uncached_and_store(ctx, is_prefetch).await;
                let mut clean_resp = ctx.response.clone();
                if let Some(ref mut r) = clean_resp {
                    r.edns = None;
                }
                let snapshot = InflightSnapshot {
                    response: clean_resp,
                    status: ctx.status.clone(),
                    routing_group: ctx.routing_group.clone(),
                    fastest_ip: ctx.fastest_ip,
                    tags: ctx.tags.clone(),
                };
                let _ = tx.send(Some(snapshot));
            }
            FlightRole::Follower(mut rx) => {
                if let Ok(snap_ref) = rx.wait_for(|v| v.is_some()).await {
                    if let Some(snap) = snap_ref.as_ref().cloned() {
                        drop(snap_ref);
                        if let Some(mut resp) = snap.response {
                            resp.metadata.id = ctx.request.metadata.id;
                            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                            resp.metadata.recursion_available = true;
                            resp.queries = ctx.request.queries.clone();
                            ctx.response = Some(resp);
                            self.apply_response_mode(ctx);
                        }
                        ctx.status = snap.status;
                        ctx.routing_group = snap.routing_group;
                        ctx.fastest_ip = snap.fastest_ip;
                        for t in snap.tags {
                            ctx.tags.insert(t);
                        }
                        ctx.tags.insert("coalesced_inflight".to_string());
                        return;
                    }
                }
                // Leader 被提前取消时安全回退自行解析，确保下游请求绝不丢失
                self.resolve_uncached_and_store(ctx, is_prefetch).await;
            }
        }
    }

    pub async fn process(&self, ctx: &mut QueryContext) {
        ctx.record_timing("start");

        // 1. 畸形域名拦截及 RFC 6761 / RFC 6303 / RFC 9462 / RFC 7050 特殊标准域前置本地响应
        let qname_lower = ctx.qname.to_ascii_lowercase();
        if qname_lower.contains(':') || qname_lower.contains('/') || qname_lower.contains('\\') || qname_lower.contains(' ') {
            let mut resp = Message::error_msg(
                ctx.request.metadata.id,
                ctx.request.metadata.op_code,
                ResponseCode::NXDomain,
            );
            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            resp.metadata.recursion_available = true;
            resp.queries = ctx.request.queries.clone();
            attach_negative_soa(&mut resp, &qname_lower, 300);
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Allowed;
            ctx.routing_group = Some("local_rfc".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // RFC 6761 Section 6.3: localhost 与 *.localhost 必须本地直接解析为 127.0.0.1 / ::1，严禁外发公网
        if qname_lower == "localhost" || qname_lower.ends_with(".localhost") {
            use std::str::FromStr;
            let mut resp = Message::new(
                ctx.request.metadata.id,
                hickory_proto::op::MessageType::Response,
                ctx.request.metadata.op_code,
            );
            resp.metadata.authoritative = true;
            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            resp.metadata.recursion_available = true;
            resp.metadata.response_code = ResponseCode::NoError;
            resp.queries = ctx.request.queries.clone();
            if let Ok(name) = hickory_proto::rr::Name::from_str(&format!("{}.", ctx.qname)) {
                match ctx.qtype {
                    RecordType::A => {
                        resp.answers.push(hickory_proto::rr::Record::from_rdata(
                            name,
                            300,
                            RData::A(hickory_proto::rr::rdata::A(std::net::Ipv4Addr::LOCALHOST)),
                        ));
                    }
                    RecordType::AAAA => {
                        resp.answers.push(hickory_proto::rr::Record::from_rdata(
                            name,
                            300,
                            RData::AAAA(hickory_proto::rr::rdata::AAAA(std::net::Ipv6Addr::LOCALHOST)),
                        ));
                    }
                    _ => {
                        attach_negative_soa(&mut resp, "localhost", 300);
                    }
                }
            }
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Allowed;
            ctx.routing_group = Some("local_rfc".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // RFC 6303 Section 4.1 / 4.2: 127.0.0.1 与 ::1 回环地址反向 PTR 本地直接返回 localhost.
        if qname_lower == "1.0.0.127.in-addr.arpa"
            || qname_lower == "1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.ip6.arpa"
        {
            use std::str::FromStr;
            let mut resp = Message::new(
                ctx.request.metadata.id,
                hickory_proto::op::MessageType::Response,
                ctx.request.metadata.op_code,
            );
            resp.metadata.authoritative = true;
            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            resp.metadata.recursion_available = true;
            resp.metadata.response_code = ResponseCode::NoError;
            resp.queries = ctx.request.queries.clone();
            if ctx.qtype == RecordType::PTR {
                if let (Ok(name), Ok(target)) = (
                    hickory_proto::rr::Name::from_str(&format!("{}.", ctx.qname)),
                    hickory_proto::rr::Name::from_str("localhost."),
                ) {
                    resp.answers.push(hickory_proto::rr::Record::from_rdata(
                        name,
                        300,
                        RData::PTR(hickory_proto::rr::rdata::PTR(target)),
                    ));
                }
            } else {
                attach_negative_soa(&mut resp, &qname_lower, 300);
            }
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Allowed;
            ctx.routing_group = Some("local_rfc".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // RFC 9462 (_dns.resolver.arpa) 与 RFC 7050 / RFC 8880 (ipv4only.arpa) 本地合成应答（附带 SOA 供客户端缓存）
        if qname_lower == "_dns.resolver.arpa" || qname_lower == "ipv4only.arpa" {
            use std::str::FromStr;
            let mut resp = Message::new(
                ctx.request.metadata.id,
                hickory_proto::op::MessageType::Response,
                ctx.request.metadata.op_code,
            );
            resp.metadata.authoritative = true;
            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            resp.metadata.recursion_available = true;
            resp.metadata.response_code = ResponseCode::NoError;
            resp.queries = ctx.request.queries.clone();
            if qname_lower == "ipv4only.arpa" && ctx.qtype == RecordType::A {
                if let Ok(name) = hickory_proto::rr::Name::from_str("ipv4only.arpa.") {
                    resp.answers.push(hickory_proto::rr::Record::from_rdata(
                        name.clone(),
                        300,
                        RData::A(hickory_proto::rr::rdata::A(std::net::Ipv4Addr::new(192, 0, 0, 170))),
                    ));
                    resp.answers.push(hickory_proto::rr::Record::from_rdata(
                        name,
                        300,
                        RData::A(hickory_proto::rr::rdata::A(std::net::Ipv4Addr::new(192, 0, 0, 171))),
                    ));
                }
            } else {
                attach_negative_soa(&mut resp, &qname_lower, 300);
            }
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Allowed;
            ctx.routing_group = Some("local_rfc".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // 2. Local Rewrites / Hosts
        ctx.record_timing("rewrite");
        if self.rewrite.execute(ctx).await == crate::pipeline::plugin::PluginResult::Accept {
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // 4. Adblock Rules (Security Filter)
        ctx.record_timing("adblock");
        if self.adblock_enabled.load(Ordering::Relaxed) {
            if self.adblock.execute(ctx).await == crate::pipeline::plugin::PluginResult::Accept {
                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        }

        // 4.5. 下游单栈模式前置短路：若配置仅返回 IPv4 (或仅 IPv6)，直接对 AAAA (或 A) 返回空 NoError (NODATA) + SOA
        {
            let mode = self.response_mode.read().clone();
            let short_circuit = matches!(
                (mode.as_str(), ctx.qtype),
                ("ipv4" | "ipv4_only", RecordType::AAAA) | ("ipv6" | "ipv6_only", RecordType::A)
            );
            if short_circuit {
                let mut resp = Message::new(
                    ctx.request.metadata.id,
                    hickory_proto::op::MessageType::Response,
                    ctx.request.metadata.op_code,
                );
                resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                resp.metadata.recursion_available = true;
                resp.metadata.response_code = ResponseCode::NoError;
                resp.queries = ctx.request.queries.clone();
                attach_negative_soa(&mut resp, &qname_lower, 300);
                ctx.response = Some(resp);
                ctx.status = QueryStatus::Allowed;
                ctx.tags.insert(format!("response_mode_filtered:{}", mode));
                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        }

        // 5. Cache (命中极速秒返；剩余 TTL < 20% 时后台完整复用分流+防污染+response_mode+测速流水线静默预取)
        ctx.record_timing("cache");
        if self.cache_enabled.load(Ordering::Relaxed) {
            if let Some((mut cached_resp, should_prefetch)) = self.cache.get_with_prefetch(&ctx.qname, ctx.qtype) {
                cached_resp.metadata.id = ctx.request.metadata.id;
                cached_resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                cached_resp.metadata.recursion_available = true;

                ctx.response = Some(cached_resp);
                self.apply_response_mode(ctx);
                ctx.status = QueryStatus::Cached;
                ctx.tags.insert("hit_cache".to_string());

                if should_prefetch {
                    ctx.tags.insert("prefetch_triggered".to_string());
                    let engine = self.clone();
                    let qname = ctx.qname.clone();
                    let qtype = ctx.qtype;
                    tokio::spawn(async move {
                        use std::str::FromStr;
                        let mut req = Message::query();
                        if let Ok(name) = hickory_proto::rr::Name::from_str(&format!("{}.", qname)) {
                            req.add_query(hickory_proto::op::Query::query(name, qtype));
                            let mut sub_ctx = QueryContext::new(
                                IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                                req,
                            );
                            engine.resolve_with_singleflight(&mut sub_ctx, true).await;
                            if sub_ctx.response.is_some() {
                                tracing::debug!(
                                    "Prefetch-Domain 自动预取静默刷新完成: {} (group={:?})",
                                    qname,
                                    sub_ctx.routing_group
                                );
                            }
                        }
                    });
                }

                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        }

        // 6-9. Singleflight 合并去重 + 智能分流 + 防污染 + 下游地址族过滤 + 测速优选 + 缓存入库
        self.resolve_with_singleflight(ctx, false).await;

        align_edns_response(ctx);
        ctx.finish();
    }
}

/// RFC 6891 / RFC 7873 / RFC 9018 EDNS 与 Cookie 协议对齐治理函数
pub fn align_edns_response(ctx: &mut QueryContext) {
    if let Some(ref mut resp) = ctx.response {
        if let Some(ref req_edns) = ctx.request.edns {
            let safe_payload = req_edns.max_payload().clamp(512, 1232);
            let client_cookie = req_edns
                .options()
                .get(hickory_proto::rr::rdata::opt::EdnsCode::Cookie)
                .cloned();

            if let Some(ref mut resp_edns) = resp.edns {
                resp_edns.set_max_payload(safe_payload);
                resp_edns.options_mut().remove(hickory_proto::rr::rdata::opt::EdnsCode::Cookie);
                if let Some(cookie) = client_cookie {
                    resp_edns.options_mut().insert(cookie);
                }
            } else if let Some(cookie) = client_cookie {
                let mut edns = hickory_proto::op::Edns::new();
                edns.set_max_payload(safe_payload);
                edns.options_mut().insert(cookie);
                resp.set_edns(edns);
            } else {
                let mut edns = hickory_proto::op::Edns::new();
                edns.set_max_payload(safe_payload);
                resp.set_edns(edns);
            }
        } else {
            resp.edns = None;
        }
    }
}

/// RFC 2308 Section 3 & Section 5: 为本地生成的 NXDOMAIN / NODATA 否定应答注入 Authority SOA 记录
pub fn attach_negative_soa(resp: &mut Message, qname: &str, ttl: u32) {
    use std::str::FromStr;
    resp.metadata.authoritative = true;
    if !resp.authorities.is_empty() {
        return;
    }
    let zone_str = if let Some(idx) = qname.find('.') {
        format!("{}.", &qname[idx + 1..].trim_end_matches('.'))
    } else {
        format!("{}.", qname.trim_end_matches('.'))
    };
    let zone = hickory_proto::rr::Name::from_str(&zone_str)
        .or_else(|_| hickory_proto::rr::Name::from_str("."))
        .unwrap();
    let mname = hickory_proto::rr::Name::from_str("ns.local.").unwrap();
    let rname = hickory_proto::rr::Name::from_str("hostmaster.local.").unwrap();
    let soa = hickory_proto::rr::rdata::SOA::new(
        mname,
        rname,
        1,
        3600,
        600,
        86400,
        ttl,
    );
    resp.authorities.push(hickory_proto::rr::Record::from_rdata(
        zone,
        ttl,
        RData::SOA(soa),
    ));
}

/// RFC 6303 (Locally Served DNS Zones) + RFC 6761 / RFC 6762 / RFC 6763 / RFC 7686 / RFC 8375 特殊与保留域判定
pub fn is_rfc_locally_served_domain(qname_lower: &str) -> bool {
    if !qname_lower.contains('.') {
        return true;
    }
    const SPECIAL_TLDS: &[&str] = &[
        ".test",
        ".invalid",
        ".local",
        ".home.arpa",
        ".internal",
        ".lan",
        ".home",
        ".corp",
        ".localdomain",
        ".onion",
    ];
    if SPECIAL_TLDS.iter().any(|tld| qname_lower.ends_with(tld)) {
        return true;
    }
    if qname_lower.contains("._dns-sd._udp.") {
        return true;
    }
    const EXTRA_RFC6303_ZONES: &[&str] = &[
        ".0.in-addr.arpa",
        ".0.0.192.in-addr.arpa",
        ".18.198.in-addr.arpa",
        ".19.198.in-addr.arpa",
        ".c.f.ip6.arpa",
        ".d.f.ip6.arpa",
    ];
    if qname_lower == "0.in-addr.arpa"
        || qname_lower == "c.f.ip6.arpa"
        || qname_lower == "d.f.ip6.arpa"
        || qname_lower == "0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.ip6.arpa"
        || EXTRA_RFC6303_ZONES.iter().any(|z| qname_lower.ends_with(z))
    {
        return true;
    }
    false
}
