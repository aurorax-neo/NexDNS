use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::{RData, RecordType};
use crate::pipeline::context::{QueryContext, QueryStatus};
use crate::pipeline::plugin::DnsPlugin;
use crate::pipeline::plugins::adblock::AdblockPlugin;
use crate::pipeline::plugins::cache::CachePlugin;
use crate::pipeline::plugins::domain_match::DomainListRegistry;
use crate::pipeline::plugins::ip_match::IpListRegistry;
use crate::pipeline::plugins::qtype_drop::QtypeDropPlugin;
use crate::pipeline::plugins::rewrite::RewritePlugin;
use crate::pipeline::plugins::speed_check::SpeedCheckPlugin;
use crate::pipeline::plugins::upstream::ForwardPlugin;

pub struct PipelineEngine {
    pub qtype_drop: Arc<QtypeDropPlugin>,
    pub rewrite: Arc<RewritePlugin>,
    pub adblock: Arc<AdblockPlugin>,
    pub cache: Arc<CachePlugin>,
    pub domain_registry: Arc<DomainListRegistry>,
    pub ip_registry: Arc<IpListRegistry>,
    pub forward_mainland: Arc<ForwardPlugin>,
    pub forward_gfw: Arc<ForwardPlugin>,
    pub forward_overseas: Arc<ForwardPlugin>,
    pub speed_check: Arc<SpeedCheckPlugin>,
    pub qtype_65_enabled: AtomicBool,
    pub adblock_enabled: AtomicBool,
    pub cache_enabled: AtomicBool,
    pub router_enabled: AtomicBool,
    pub anti_pollution_enabled: AtomicBool,
    pub speed_check_enabled: AtomicBool,
    pub response_mode: Arc<parking_lot::RwLock<String>>,
}

impl PipelineEngine {
    pub fn new(
        qtype_drop: Arc<QtypeDropPlugin>,
        rewrite: Arc<RewritePlugin>,
        adblock: Arc<AdblockPlugin>,
        cache: Arc<CachePlugin>,
        domain_registry: Arc<DomainListRegistry>,
        ip_registry: Arc<IpListRegistry>,
        forward_mainland: Arc<ForwardPlugin>,
        forward_gfw: Arc<ForwardPlugin>,
        forward_overseas: Arc<ForwardPlugin>,
        speed_check: Arc<SpeedCheckPlugin>,
        qtype_65_enabled: bool,
        router_enabled: bool,
        anti_pollution_enabled: bool,
        response_mode: String,
    ) -> Self {
        Self {
            qtype_drop,
            rewrite,
            adblock,
            cache,
            domain_registry,
            ip_registry,
            forward_mainland,
            forward_gfw,
            forward_overseas,
            speed_check,
            qtype_65_enabled: AtomicBool::new(qtype_65_enabled),
            adblock_enabled: AtomicBool::new(true),
            cache_enabled: AtomicBool::new(true),
            router_enabled: AtomicBool::new(router_enabled),
            anti_pollution_enabled: AtomicBool::new(anti_pollution_enabled),
            speed_check_enabled: AtomicBool::new(true),
            response_mode: Arc::new(parking_lot::RwLock::new(response_mode)),
        }
    }

    pub fn toggle_stage(&self, stage_id: &str, enabled: bool) -> bool {
        match stage_id {
            "qtype_drop" => {
                self.qtype_65_enabled.store(enabled, Ordering::SeqCst);
                true
            }
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
                self.speed_check_enabled.store(enabled, Ordering::SeqCst);
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

    pub fn has_china_ip(&self, resp: &Message) -> bool {
        for ans in &resp.answers {
            match &ans.data {
                RData::A(a) => {
                    let ip = std::net::IpAddr::V4(a.0);
                    if self.ip_registry.contains("ipv4_china", &ip) {
                        return true;
                    }
                }
                RData::AAAA(aaaa) => {
                    let ip = std::net::IpAddr::V6(aaaa.0);
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

    pub async fn process(&self, ctx: &mut QueryContext) {
        ctx.record_timing("start");

        // 1. Qtype check (e.g. Qtype 65 drop)
        if self.qtype_65_enabled.load(Ordering::Relaxed) && ctx.qtype == RecordType::from(65) {
            self.qtype_drop.execute(ctx).await;
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // 2. Private domain check
        if self.domain_registry.matches("domain_private", &ctx.qname) {
            let mut resp = Message::error_msg(
                ctx.request.metadata.id,
                ctx.request.metadata.op_code,
                ResponseCode::Refused,
            );
            resp.queries = ctx.request.queries.clone();
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Blocked;
            ctx.matched_rule = Some("domain_private".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // 3. Local Rewrites / Hosts
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

        // 5. Cache (Serve-expired)
        ctx.record_timing("cache");
        if self.cache_enabled.load(Ordering::Relaxed) {
            if let Some((mut cached_resp, is_expired)) = self.cache.get(&ctx.qname, ctx.qtype) {
                cached_resp.metadata.id = ctx.request.metadata.id;
                cached_resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                cached_resp.metadata.recursion_available = true;

                ctx.response = Some(cached_resp);
                ctx.status = QueryStatus::Cached;
                if is_expired {
                    ctx.tags.insert("hit_cache_expired".to_string());
                    self.spawn_background_refresh(ctx.qname.clone(), ctx.qtype);
                } else {
                    ctx.tags.insert("hit_cache".to_string());
                }
                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        }

        // 6. Router & Dispatch (Routing pipeline)
        ctx.record_timing("route");
        if !self.router_enabled.load(Ordering::Relaxed) {
            ctx.tags.insert("route:bypass_disabled".to_string());
            self.forward_mainland.execute(ctx).await;
        } else {
        let is_direct = self.domain_registry.matches("domain_direct", &ctx.qname);
        let is_proxy = self.domain_registry.matches("domain_proxy", &ctx.qname);

        if is_direct {
            ctx.tags.insert("route:direct".to_string());
            self.forward_mainland.execute(ctx).await;
        } else if is_proxy {
            ctx.tags.insert("route:proxy".to_string());
            self.forward_gfw.execute(ctx).await;
        } else {
            // Fallback pipeline: try mainland first
            ctx.tags.insert("route:fallback".to_string());
            self.forward_mainland.execute(ctx).await;

            let mut mainland_fallback_resp = None;

            // Anti-pollution check: if mainland returned an IP that is NOT in China IP list, drop response and fallback to GFW!
            if self.anti_pollution_enabled.load(Ordering::Relaxed) {
                if let Some(ref resp) = ctx.response {
                    if self.has_ip_answers(resp) && !self.has_china_ip(resp) {
                        ctx.tags.insert("pollution_detected".to_string());
                        mainland_fallback_resp = ctx.response.take(); // 暂存大陆原始响应以供降级兜底，清空当前响应触发后续 GFW 探测
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

                // 关键容灾兜底: 如果 GFW 与 Overseas 境外上游全部超时、熔断或失败，
                // 且此前国内上游已拿到响应，则立即启用国内原始响应作为保底恢复，
                // 坚决防止向客户端抛出 Server Failure 导致断网！
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

        // 7. Speed Check (Fastest IP)
        if ctx.response.is_some() {
            if self.speed_check_enabled.load(Ordering::Relaxed) {
                ctx.record_timing("speed_check");
                self.speed_check.execute(ctx).await;
            }

            // 8. Cache storage
            if self.cache_enabled.load(Ordering::Relaxed) {
                if let Some(ref resp) = ctx.response {
                    if resp.metadata.response_code == ResponseCode::NoError {
                        self.cache.insert(&ctx.qname, ctx.qtype, resp.clone());
                    }
                }
            }

            // 9. Downstream response filtering (IPv4 / IPv6 / Dual stack)
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
        } else {
            let mut resp = Message::error_msg(
                ctx.request.metadata.id,
                ctx.request.metadata.op_code,
                ResponseCode::ServFail,
            );
            resp.queries = ctx.request.queries.clone();
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Error;
        }

        align_edns_response(ctx);
        ctx.finish();
    }

    pub fn spawn_background_refresh(&self, qname: String, qtype: RecordType) {
        let is_direct = self.domain_registry.matches("domain_direct", &qname);
        let is_proxy = self.domain_registry.matches("domain_proxy", &qname);
        let forward_mainland = self.forward_mainland.clone();
        let forward_gfw = self.forward_gfw.clone();
        let forward_overseas = self.forward_overseas.clone();
        let cache = self.cache.clone();
        let speed_check = self.speed_check.clone();
        let speed_check_enabled = self.speed_check_enabled.load(Ordering::Relaxed);
        let router_enabled = self.router_enabled.load(Ordering::Relaxed);
        let anti_pollution_enabled = self.anti_pollution_enabled.load(Ordering::Relaxed);
        let ip_registry = self.ip_registry.clone();

        tokio::spawn(async move {
            use std::str::FromStr;
            let mut req = Message::query();
            let Ok(name) = hickory_proto::rr::Name::from_str(&format!("{}.", qname.trim_end_matches('.'))) else {
                return;
            };
            req.add_query(hickory_proto::op::Query::query(name, qtype));
            let dummy_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
            let mut bg_ctx = QueryContext::new(dummy_ip, req);

            if !router_enabled {
                forward_mainland.execute(&mut bg_ctx).await;
            } else if is_direct {
                forward_mainland.execute(&mut bg_ctx).await;
            } else if is_proxy {
                forward_gfw.execute(&mut bg_ctx).await;
            } else {
                forward_mainland.execute(&mut bg_ctx).await;
                let mut bg_fallback_resp = None;
                if anti_pollution_enabled {
                    if let Some(ref resp) = bg_ctx.response {
                        let mut has_ip = false;
                        let mut has_cn = false;
                        for ans in &resp.answers {
                            match &ans.data {
                                RData::A(a) => {
                                    has_ip = true;
                                    if ip_registry.contains("ipv4_china", &std::net::IpAddr::V4(a.0)) {
                                        has_cn = true;
                                    }
                                }
                                RData::AAAA(aaaa) => {
                                    has_ip = true;
                                    if ip_registry.contains("ipv6_china", &std::net::IpAddr::V6(aaaa.0)) {
                                        has_cn = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                        if has_ip && !has_cn {
                            bg_fallback_resp = bg_ctx.response.take();
                        }
                    }
                }
                if bg_ctx.response.is_none() {
                    forward_gfw.execute(&mut bg_ctx).await;
                    if bg_ctx.response.is_none() {
                        forward_overseas.execute(&mut bg_ctx).await;
                    }
                    if bg_ctx.response.is_none() && bg_fallback_resp.is_some() {
                        bg_ctx.response = bg_fallback_resp;
                    }
                }
            }

            if bg_ctx.response.is_some() {
                if speed_check_enabled {
                    speed_check.execute(&mut bg_ctx).await;
                }
                if let Some(resp) = bg_ctx.response {
                    if resp.metadata.response_code == ResponseCode::NoError {
                        cache.insert(&qname, qtype, resp);
                    }
                }
            }
        });
    }
}

/// RFC 6891 / RFC 7873 / RFC 9018 EDNS 与 Cookie 协议对齐治理函数
/// 彻底消除所有下游客户端出现 `;; Warning: Client COOKIE mismatch` 的根本原因：
/// 1. 若下游请求携带了 EDNS Cookie，应答必须精准回显客户端自生成的 8 字节 Client Cookie；
/// 2. 若下游请求未携带 Cookie，应答绝对不返回上游或缓存带入的异构 Cookie；
/// 3. 若下游客户端未开启 EDNS，应答严格遵循 RFC 6891 清除 OPT 伪记录。
pub fn align_edns_response(ctx: &mut QueryContext) {
    if let Some(ref mut resp) = ctx.response {
        if let Some(ref req_edns) = ctx.request.edns {
            let client_cookie = req_edns
                .options()
                .get(hickory_proto::rr::rdata::opt::EdnsCode::Cookie)
                .cloned();

            if let Some(ref mut resp_edns) = resp.edns {
                resp_edns.options_mut().remove(hickory_proto::rr::rdata::opt::EdnsCode::Cookie);
                if let Some(cookie) = client_cookie {
                    resp_edns.options_mut().insert(cookie);
                }
            } else if let Some(cookie) = client_cookie {
                let mut edns = hickory_proto::op::Edns::new();
                edns.set_max_payload(req_edns.max_payload().clamp(512, 4096));
                edns.options_mut().insert(cookie);
                resp.set_edns(edns);
            } else {
                let mut edns = hickory_proto::op::Edns::new();
                edns.set_max_payload(req_edns.max_payload().clamp(512, 4096));
                resp.set_edns(edns);
            }
        } else {
            resp.edns = None;
        }
    }
}
