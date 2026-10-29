use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Instant;
use chrono::{DateTime, Utc};
use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::RecordType;
use uuid::Uuid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum QueryStatus {
    Allowed,
    Blocked,
    Cached,
    Rewritten,
    Error,
}

#[derive(Debug, Clone)]
pub struct QueryContext {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub client_ip: IpAddr,
    pub client_name: Option<String>,
    pub qname: String,
    pub qtype: RecordType,
    pub request: Message,
    pub response: Option<Message>,
    pub status: QueryStatus,
    pub rcode: ResponseCode,
    pub tags: HashSet<String>,
    pub matched_rule: Option<String>,
    pub routing_group: Option<String>,
    pub fastest_ip: Option<IpAddr>,
    pub timings: Vec<(String, u64)>,
    pub start_time: Instant,
    pub duration_ms: f64,
}

impl QueryContext {
    pub fn new(client_ip: IpAddr, request: Message) -> Self {
        let qname = request
            .queries
            .first()
            .map(|q| q.name().to_utf8().trim_end_matches('.').to_lowercase())
            .unwrap_or_default();
        let qtype = request
            .queries
            .first()
            .map(|q| q.query_type())
            .unwrap_or(RecordType::A);

        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            client_ip,
            client_name: None,
            qname,
            qtype,
            request,
            response: None,
            status: QueryStatus::Allowed,
            rcode: ResponseCode::NoError,
            tags: HashSet::new(),
            matched_rule: None,
            routing_group: None,
            fastest_ip: None,
            timings: Vec::new(),
            start_time: Instant::now(),
            duration_ms: 0.0,
        }
    }

    pub fn record_timing(&mut self, stage: &str) {
        let elapsed = self.start_time.elapsed().as_micros() as u64;
        self.timings.push((stage.to_string(), elapsed));
    }

    pub fn finish(&mut self) {
        self.duration_ms = self.start_time.elapsed().as_secs_f64() * 1000.0;
        if let Some(ref res) = self.response {
            self.rcode = res.metadata.response_code;
        }
    }
}
