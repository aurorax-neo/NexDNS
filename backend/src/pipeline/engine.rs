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
use crate::pipeline::plugins::rewrite::RewritePlugin;
use crate::pipeline::plugins::speed_check::SpeedCheckPlugin;
use crate::pipeline::plugins::upstream::ForwardPlugin;

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
    pub adblock_enabled: AtomicBool,
    pub cache_enabled: AtomicBool,
    pub router_enabled: AtomicBool,
    pub anti_pollution_enabled: AtomicBool,
    pub speed_check_enabled: AtomicBool,
    pub response_mode: Arc<parking_lot::RwLock<String>>,
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

    pub async fn process(&self, ctx: &mut QueryContext) {
        ctx.record_timing("start");

        // 1. 畸形域名/URL 拦截与 RFC 本地特殊发现域响应 (_dns.resolver.arpa / ipv4only.arpa)
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
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Allowed;
            ctx.routing_group = Some("local_rfc".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }
        if qname_lower == "_dns.resolver.arpa" || qname_lower == "ipv4only.arpa" {
            let mut resp = Message::new(
                ctx.request.metadata.id,
                hickory_proto::op::MessageType::Response,
                ctx.request.metadata.op_code,
            );
            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            resp.metadata.recursion_available = true;
            resp.metadata.response_code = ResponseCode::NoError;
            resp.queries = ctx.request.queries.clone();
            ctx.response = Some(resp);
            ctx.status = QueryStatus::Allowed;
            ctx.routing_group = Some("local_rfc".to_string());
            align_edns_response(ctx);
            ctx.finish();
            return;
        }

        // 2. Local Rewrites / Hosts（优先于拦截与私有域规则，支持将 tplinkac.tplogin.cn / nas.home 等重写到内网 IP）
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

        // 4.5. 下游单栈模式前置短路：若配置仅返回 IPv4 (或仅 IPv6)，直接对 AAAA (或 A) 返回空 NoError (NODATA)
        // 避免无 IPv6 出口的楼层因缓存命中返回 AAAA 记录导致双栈 Happy Eyeballs 退避卡顿，同时节省 50% 无效回源
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
                ctx.response = Some(resp);
                ctx.status = QueryStatus::Allowed;
                ctx.tags.insert(format!("response_mode_filtered:{}", mode));
                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        }

        // 5. Cache
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
                    let is_proxy_domain = self.router_enabled.load(Ordering::Relaxed)
                        && self.domain_registry.matches("domain_proxy", &ctx.qname);
                    let forward_primary = if is_proxy_domain {
                        self.forward_gfw.clone()
                    } else {
                        self.forward_mainland.clone()
                    };
                    let forward_fallback = self.forward_mainland.clone();
                    let cache = self.cache.clone();
                    let qname = ctx.qname.clone();
                    let qtype = ctx.qtype;
                    tokio::spawn(async move {
                        use std::str::FromStr;
                        let mut req = Message::query();
                        if let Ok(name) = hickory_proto::rr::Name::from_str(&format!("{}.", qname)) {
                            req.add_query(hickory_proto::op::Query::query(name, qtype));
                            let mut sub_ctx = QueryContext::new(
                                std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                                req,
                            );
                            forward_primary.execute(&mut sub_ctx).await;
                            let ok = sub_ctx
                                .response
                                .as_ref()
                                .map(|r| r.metadata.response_code == ResponseCode::NoError && !r.answers.is_empty())
                                .unwrap_or(false);
                            if !ok && is_proxy_domain {
                                forward_fallback.execute(&mut sub_ctx).await;
                            }
                            if let Some(resp) = sub_ctx.response {
                                if resp.metadata.response_code == ResponseCode::NoError && !resp.answers.is_empty() {
                                    cache.insert(&qname, qtype, resp);
                                    tracing::debug!("Prefetch-Domain 自动预取静默更新成功: {}", qname);
                                }
                            }
                        }
                    });
                }

                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        }

        // 6. Router & Dispatch (Routing pipeline)
        ctx.record_timing("route");
        if self.domain_registry.matches("domain_private", &ctx.qname) {
            // 区分 RoutingRules private.txt 中的两类条目：
            // (a) 需走大陆公网 DNS 解析出回环/内网直连 IP 的应用辅助域（如 localhost.weixin.qq.com、localhost.ptlogin2.qq.com、plex.direct、ts.net）
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
                // (b) RFC 6303 私有反向解析域 (*.in-addr.arpa / *.ip6.arpa)、单标签主机名及未配置重写的路由器/AP发现域 (如 tplinkac.tplogin.cn)：
                // 本地直接返回标准 NXDomain（标记为 Allowed 并写入 60s 否定缓存），不向公网泄露，也不返回 Refused 触发客户端每秒死循环重试
                let mut resp = Message::error_msg(
                    ctx.request.metadata.id,
                    ctx.request.metadata.op_code,
                    ResponseCode::NXDomain,
                );
                resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                resp.metadata.recursion_available = true;
                resp.queries = ctx.request.queries.clone();
                if self.cache_enabled.load(Ordering::Relaxed) {
                    self.cache.insert(&ctx.qname, ctx.qtype, resp.clone());
                }
                ctx.response = Some(resp);
                ctx.status = QueryStatus::Allowed;
                ctx.routing_group = Some("private_local".to_string());
                align_edns_response(ctx);
                ctx.finish();
                return;
            }
        } else if !self.router_enabled.load(Ordering::Relaxed) {
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

            // 容灾兜底：若代理未开启或境外上游不可达，自动降级至大陆上游解析，防止 proxy.txt 域名直接返回 ServFail 断网
            if !overseas_ok {
                ctx.tags.insert("fallback_restore_mainland".to_string());
                self.forward_mainland.execute(ctx).await;
            }
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
            // 8. 先执行下游地址族过滤，再测速与写缓存，杜绝被过滤的 AAAA/A 记录污染缓存
            self.apply_response_mode(ctx);

            if self.speed_check_enabled.load(Ordering::Relaxed) {
                ctx.record_timing("speed_check");
                self.speed_check.execute(ctx).await;
            }

            // 9. Cache storage
            if self.cache_enabled.load(Ordering::Relaxed) {
                if let Some(ref resp) = ctx.response {
                    if resp.metadata.response_code == ResponseCode::NoError
                        || resp.metadata.response_code == ResponseCode::NXDomain
                    {
                        self.cache.insert(&ctx.qname, ctx.qtype, resp.clone());
                    }
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
}

/// RFC 6891 / RFC 7873 / RFC 9018 EDNS 与 Cookie 协议对齐治理函数
/// 彻底消除所有下游客户端出现 `;; Warning: Client COOKIE mismatch` 的根本原因：
/// 1. 若下游请求携带了 EDNS Cookie，应答必须精准回显客户端自生成的 8 字节 Client Cookie；
/// 2. 若下游请求未携带 Cookie，应答绝对不返回上游或缓存带入的异构 Cookie；
/// 3. 若下游客户端未开启 EDNS，应答严格遵循 RFC 6891 清除 OPT 伪记录。
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
