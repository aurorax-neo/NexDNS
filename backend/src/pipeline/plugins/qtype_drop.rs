use async_trait::async_trait;
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::RecordType;
use crate::pipeline::context::QueryContext;
use crate::pipeline::plugin::{DnsPlugin, PluginResult};

pub struct QtypeDropPlugin {
    pub target_qtype: RecordType,
    pub rcode: ResponseCode,
}

impl QtypeDropPlugin {
    pub fn new(qtype_u16: u16, rcode: ResponseCode) -> Self {
        Self {
            target_qtype: RecordType::from(qtype_u16),
            rcode,
        }
    }
}

#[async_trait]
impl DnsPlugin for QtypeDropPlugin {
    fn name(&self) -> &str {
        "qtype_drop"
    }

    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult {
        if ctx.qtype == self.target_qtype {
            let mut resp = Message::response(ctx.request.metadata.id, ctx.request.metadata.op_code);
            resp.metadata.recursion_desired = ctx.request.metadata.recursion_desired;
            resp.metadata.recursion_available = true;
            resp.metadata.response_code = self.rcode;
            resp.queries = ctx.request.queries.clone();

            ctx.response = Some(resp);
            ctx.matched_rule = Some(format!("qtype_{}", u16::from(self.target_qtype)));
            ctx.tags.insert("dropped_qtype".to_string());
            PluginResult::Accept
        } else {
            PluginResult::Continue
        }
    }
}
