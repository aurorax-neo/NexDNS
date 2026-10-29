use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::Arc;
use async_trait::async_trait;
use parking_lot::RwLock;
use regex::Regex;
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::rdata::{A, AAAA};
use hickory_proto::rr::{Name, RData, Record, RecordType};
use crate::pipeline::context::{QueryContext, QueryStatus};
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BlockType {
    ZeroIp,
    NxDomain,
    Refused,
}

#[derive(Default)]
pub struct RuleStore {
    pub exact_deny: HashSet<String>,
    pub suffix_deny: HashSet<String>,
    pub exact_allow: HashSet<String>,
    pub suffix_allow: HashSet<String>,
    pub regex_deny: Vec<Regex>,
    pub total_count: usize,
}

impl RuleStore {
    pub fn parse_rules(&mut self, text: &str) {
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                continue;
            }

            // Whitelist syntax: @@||domain^ or @@domain
            if let Some(rest) = trimmed.strip_prefix("@@") {
                if let Some(domain) = rest.strip_prefix("||") {
                    let domain = domain.trim_end_matches('^').trim_end_matches('.');
                    self.suffix_allow.insert(domain.to_lowercase());
                } else {
                    let domain = rest.trim_end_matches('^').trim_end_matches('.');
                    self.exact_allow.insert(domain.to_lowercase());
                }
                self.total_count += 1;
                continue;
            }

            // Regex syntax: /pattern/
            if trimmed.starts_with('/') && trimmed.ends_with('/') && trimmed.len() > 2 {
                let pattern = &trimmed[1..trimmed.len() - 1];
                if let Ok(re) = Regex::new(pattern) {
                    self.regex_deny.push(re);
                    self.total_count += 1;
                    continue;
                }
            }

            // ABP syntax: ||domain^
            if let Some(domain) = trimmed.strip_prefix("||") {
                let domain = domain.trim_end_matches('^').trim_end_matches('.');
                self.suffix_deny.insert(domain.to_lowercase());
                self.total_count += 1;
                continue;
            }

            // Hosts syntax: 0.0.0.0 domain or 127.0.0.1 domain
            if trimmed.starts_with("0.0.0.0 ") || trimmed.starts_with("127.0.0.1 ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let domain = parts[1].trim_end_matches('.');
                    self.exact_deny.insert(domain.to_lowercase());
                    self.total_count += 1;
                    continue;
                }
            }

            // Plain domain
            let domain = trimmed.trim_end_matches('^').trim_end_matches('.');
            if !domain.is_empty() && !domain.contains(' ') {
                self.exact_deny.insert(domain.to_lowercase());
                self.total_count += 1;
            }
        }
    }

    pub fn is_allowed(&self, qname: &str) -> bool {
        if self.exact_allow.contains(qname) {
            return true;
        }
        let mut sub = qname;
        if self.suffix_allow.contains(sub) {
            return true;
        }
        while let Some(idx) = sub.find('.') {
            sub = &sub[idx + 1..];
            if self.suffix_allow.contains(sub) {
                return true;
            }
        }
        false
    }

    pub fn match_deny(&self, qname: &str) -> Option<String> {
        if self.is_allowed(qname) {
            return None;
        }

        if self.exact_deny.contains(qname) {
            return Some(format!("exact: {}", qname));
        }

        let mut sub = qname;
        if self.suffix_deny.contains(sub) {
            return Some(format!("suffix: ||{}^", sub));
        }
        while let Some(idx) = sub.find('.') {
            sub = &sub[idx + 1..];
            if self.suffix_deny.contains(sub) {
                return Some(format!("suffix: ||{}^", sub));
            }
        }

        for re in &self.regex_deny {
            if re.is_match(qname) {
                return Some(format!("regex: /{}/", re.as_str()));
            }
        }

        None
    }
}

pub struct AdblockPlugin {
    rules: Arc<RwLock<RuleStore>>,
    block_type: BlockType,
}

impl AdblockPlugin {
    pub fn new(block_type: BlockType) -> Self {
        Self {
            rules: Arc::new(RwLock::new(RuleStore::default())),
            block_type,
        }
    }

    pub fn load_rules_from_text(&self, text: &str) {
        let mut store = RuleStore::default();
        store.parse_rules(text);
        let mut w = self.rules.write();
        *w = store;
    }

    pub fn rule_count(&self) -> usize {
        self.rules.read().total_count
    }
}

#[async_trait]
impl DnsPlugin for AdblockPlugin {
    fn name(&self) -> &str {
        "adblock"
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        let matched = {
            let r = self.rules.read();
            r.match_deny(&ctx.qname)
        };

        if let Some(rule) = matched {
            ctx.status = QueryStatus::Blocked;
            ctx.matched_rule = Some(rule);
            ctx.tags.insert("ad_blocked".to_string());

            match self.block_type {
                BlockType::NxDomain => {
                    let mut resp = Message::error_msg(
                        ctx.request.metadata.id,
                        ctx.request.metadata.op_code,
                        ResponseCode::NXDomain,
                    );
                    resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                    resp.metadata.recursion_available = true;
                    resp.queries = ctx.request.queries.clone();
                    ctx.response = Some(resp);
                    PluginResult::Accept
                }
                BlockType::Refused => {
                    let mut resp = Message::error_msg(
                        ctx.request.metadata.id,
                        ctx.request.metadata.op_code,
                        ResponseCode::Refused,
                    );
                    resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                    resp.metadata.recursion_available = true;
                    resp.queries = ctx.request.queries.clone();
                    ctx.response = Some(resp);
                    PluginResult::Accept
                }
                BlockType::ZeroIp => {
                    let mut resp = Message::response(ctx.request.metadata.id, ctx.request.metadata.op_code);
                    resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
                    resp.metadata.recursion_available = true;
                    resp.metadata.response_code = ResponseCode::NoError;
                    resp.queries = ctx.request.queries.clone();

                    if let Ok(name) = Name::from_str(&format!("{}.", ctx.qname)) {
                        if ctx.qtype == RecordType::A {
                            let record = Record::from_rdata(name, 60, RData::A(A(Ipv4Addr::new(0, 0, 0, 0))));
                            resp.answers.push(record);
                        } else if ctx.qtype == RecordType::AAAA {
                            let record = Record::from_rdata(name, 60, RData::AAAA(AAAA(Ipv6Addr::UNSPECIFIED)));
                            resp.answers.push(record);
                        }
                    }

                    ctx.response = Some(resp);
                    PluginResult::Accept
                }
            }
        } else {
            PluginResult::Continue
        }
    }
}
