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
    pub anti_pollution_enabled: bool,
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
        anti_pollution_enabled: bool,
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
            anti_pollution_enabled,
        }
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
        if ctx.qtype == RecordType::from(65) {
            self.qtype_drop.execute(ctx).await;
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
            ctx.finish();
            return;
        }

        // 3. Local Rewrites / Hosts
        ctx.record_timing("rewrite");
        if self.rewrite.execute(ctx).await == crate::pipeline::plugin::PluginResult::Accept {
            ctx.finish();
            return;
        }

        // 4. Adblock Rules (AdGuard)
        ctx.record_timing("adblock");
        if self.adblock.execute(ctx).await == crate::pipeline::plugin::PluginResult::Accept {
            ctx.finish();
            return;
        }

        // 5. Cache (SmartDNS serve-expired)
        ctx.record_timing("cache");
        if self.cache.execute(ctx).await == crate::pipeline::plugin::PluginResult::Accept {
            ctx.finish();
            return;
        }

        // 6. Router & Dispatch (MosDNS pipeline)
        ctx.record_timing("route");
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

            // Anti-pollution check: if mainland returned an IP that is NOT in China IP list, drop response and fallback to GFW!
            if self.anti_pollution_enabled {
                if let Some(ref resp) = ctx.response {
                    if self.has_ip_answers(resp) && !self.has_china_ip(resp) {
                        ctx.tags.insert("pollution_detected".to_string());
                        ctx.response = None; // Drop response
                    }
                }
            }

            // If mainland returned no response or dropped due to pollution, query GFW/Overseas
            if ctx.response.is_none() {
                ctx.tags.insert("fallback_to_gfw".to_string());
                self.forward_gfw.execute(ctx).await;
                if ctx.response.is_none() {
                    self.forward_overseas.execute(ctx).await;
                }
            }
        }

        // 7. Speed Check (SmartDNS fastest IP)
        if ctx.response.is_some() {
            ctx.record_timing("speed_check");
            self.speed_check.execute(ctx).await;

            // 8. Cache storage
            if let Some(ref resp) = ctx.response {
                if resp.metadata.response_code == ResponseCode::NoError {
                    self.cache.insert(&ctx.qname, ctx.qtype, resp.clone());
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

        ctx.finish();
    }
}
