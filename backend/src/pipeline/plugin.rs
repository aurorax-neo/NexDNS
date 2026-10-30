use async_trait::async_trait;
use hickory_proto::op::ResponseCode;
use super::context::QueryContext;

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PluginResult {
    Continue,
    Accept,
    Jump(String),
    DropResp,
    Reject(ResponseCode),
}

#[async_trait]
pub trait DnsPlugin: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;
    async fn execute(&self, ctx: &mut QueryContext) -> PluginResult;
}
