use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::Arc;
use async_trait::async_trait;
use parking_lot::RwLock;
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::rdata::{A, AAAA, CNAME};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use crate::pipeline::context::{QueryContext, QueryStatus};
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

#[derive(Debug, Clone)]
pub enum RewriteTarget {
    Ip(IpAddr),
    Cname(String),
}

pub struct RewritePlugin {
    records: Arc<RwLock<HashMap<String, Vec<RewriteTarget>>>>,
}

impl RewritePlugin {
    pub fn new() -> Self {
        let mut default_records = HashMap::new();
        default_records.insert(
            "localhost".to_string(),
            vec![
                RewriteTarget::Ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
                RewriteTarget::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)),
            ],
        );

        Self {
            records: Arc::new(RwLock::new(default_records)),
        }
    }

    pub fn set_records(&self, new_records: HashMap<String, Vec<RewriteTarget>>) {
        let mut w = self.records.write();
        *w = new_records;
    }

    pub fn add_record(&self, domain: String, target: RewriteTarget) {
        let mut w = self.records.write();
        w.entry(domain.to_lowercase()).or_default().push(target);
    }

    pub fn remove_record(&self, domain: &str) {
        let mut w = self.records.write();
        w.remove(&domain.to_lowercase());
    }
}

#[async_trait]
impl DnsPlugin for RewritePlugin {
    fn name(&self) -> &str {
        "rewrite"
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        let r = self.records.read();
        if let Some(targets) = r.get(&ctx.qname) {
            let Ok(name) = Name::from_str(&format!("{}.", ctx.qname)) else {
                return PluginResult::Continue;
            };

            let mut answers = Vec::new();
            for target in targets {
                match (target, ctx.qtype) {
                    (RewriteTarget::Ip(IpAddr::V4(ipv4)), RecordType::A) => {
                        let record = Record::from_rdata(name.clone(), 300, RData::A(A(*ipv4)));
                        answers.push(record);
                    }
                    (RewriteTarget::Ip(IpAddr::V6(ipv6)), RecordType::AAAA) => {
                        let record = Record::from_rdata(name.clone(), 300, RData::AAAA(AAAA(*ipv6)));
                        answers.push(record);
                    }
                    (RewriteTarget::Cname(cname), RecordType::CNAME) => {
                        if let Ok(cname_record) = Name::from_str(&format!("{}.", cname)) {
                            let record = Record::from_rdata(name.clone(), 300, RData::CNAME(CNAME(cname_record)));
                            answers.push(record);
                        }
                    }
                    _ => {}
                }
            }

            if !answers.is_empty() {
                let mut resp = Message::response(ctx.request.metadata.id, ctx.request.metadata.op_code);
                resp.metadata.authoritative = true;
                resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                resp.metadata.recursion_available = true;
                resp.metadata.response_code = ResponseCode::NoError;
                resp.queries = ctx.request.queries.clone();
                resp.answers = answers;

                ctx.response = Some(resp);
                ctx.status = QueryStatus::Rewritten;
                ctx.matched_rule = Some(format!("rewrite: {}", ctx.qname));
                ctx.tags.insert("rewritten".to_string());
                return PluginResult::Accept;
            }
        }

        PluginResult::Continue
    }
}
