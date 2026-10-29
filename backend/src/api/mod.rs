use std::sync::Arc;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_embed::RustEmbed;

use crate::config::AppConfig;
use crate::db::{AdblockRuleRecord, Database, DnsRewriteRecord};
use crate::metrics::MetricsCollector;
use crate::pipeline::plugin::DnsPlugin;
use crate::pipeline::engine::PipelineEngine;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct FrontendAssets;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<PipelineEngine>,
    pub db: Arc<Database>,
    pub metrics: Arc<MetricsCollector>,
    pub config: Arc<AppConfig>,
    pub start_time: std::time::Instant,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/querylog", get(get_query_logs))
        .route("/api/v1/querylog/export", get(export_query_logs_csv))
        .route("/api/v1/querylog/clear", post(clear_query_logs))
        .route("/api/v1/pipeline", get(get_pipeline))
        .route("/api/v1/rules", get(get_rules).post(add_rule))
        .route("/api/v1/rules/{id}", delete(delete_rule))
        .route("/api/v1/rules/{id}/refresh", post(refresh_rule))
        .route("/api/v1/rules/test", post(test_rule))
        .route("/api/v1/upstreams", get(get_upstreams))
        .route("/api/v1/upstreams/test", post(test_upstreams))
        .route("/api/v1/rewrites", get(get_rewrites).post(add_rewrite))
        .route("/api/v1/rewrites/{id}", delete(delete_rewrite))
        .route("/api/v1/cache/flush", post(flush_cache))
        .route("/api/v1/ws/querylog", get(ws_querylog_handler))
        .fallback(static_handler)
        .with_state(state)
}

async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match FrontendAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            ([(axum::http::header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => {
            // SPA fallback to index.html
            match FrontendAssets::get("index.html") {
                Some(content) => {
                    ([(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")], content.data).into_response()
                }
                None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
            }
        }
    }
}

#[derive(Serialize)]
struct StatusResponse {
    version: &'static str,
    uptime_seconds: u64,
    dns_port: u16,
    web_port: u16,
    cache_entries: usize,
    adblock_rules_count: usize,
    anti_pollution: bool,
}

async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        version: "0.1.0",
        uptime_seconds: state.start_time.elapsed().as_secs(),
        dns_port: state.config.dns.port,
        web_port: state.config.web.port,
        cache_entries: state.engine.cache.size(),
        adblock_rules_count: state.engine.adblock.rule_count(),
        anti_pollution: state.engine.anti_pollution_enabled,
    })
}

async fn get_metrics(State(state): State<AppState>) -> Json<crate::metrics::MetricsSnapshot> {
    Json(state.metrics.snapshot())
}

#[derive(Deserialize)]
struct QueryLogParams {
    page: Option<usize>,
    page_size: Option<usize>,
    search: Option<String>,
    status: Option<String>,
    client: Option<String>,
}

#[derive(Serialize)]
struct QueryLogResponse {
    items: Vec<crate::db::QueryLogItem>,
    total: usize,
    page: usize,
    page_size: usize,
}

async fn get_query_logs(
    State(state): State<AppState>,
    Query(params): Query<QueryLogParams>,
) -> Result<Json<QueryLogResponse>, (StatusCode, String)> {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(30).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let (items, total) = state
        .db
        .query_logs(
            page_size,
            offset,
            params.search.as_deref(),
            params.status.as_deref(),
            params.client.as_deref(),
        )
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(QueryLogResponse {
        items,
        total,
        page,
        page_size,
    }))
}

async fn export_query_logs_csv(State(state): State<AppState>) -> Response {
    let Ok((items, _)) = state.db.query_logs(5000, 0, None, None, None) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load logs").into_response();
    };

    let mut csv = String::from("id,timestamp,client_ip,client_name,qname,qtype,status,rcode,matched_rule,routing_group,response_ip,duration_ms\n");
    for item in items {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{:.2}\n",
            item.id,
            item.timestamp,
            item.client_ip,
            item.client_name.unwrap_or_default(),
            item.qname,
            item.qtype,
            item.status,
            item.rcode,
            item.matched_rule.unwrap_or_default().replace(',', " "),
            item.routing_group.unwrap_or_default(),
            item.response_ip.unwrap_or_default(),
            item.duration_ms
        ));
    }

    (
        [
            ("content-type", "text/csv; charset=utf-8"),
            ("content-disposition", "attachment; filename=\"nexdns-querylog.csv\""),
        ],
        csv,
    )
        .into_response()
}

async fn clear_query_logs(State(state): State<AppState>) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .clear_logs()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct PipelineNode {
    id: &'static str,
    name: &'static str,
    stage: &'static str,
    description: &'static str,
    enabled: bool,
}

async fn get_pipeline(State(state): State<AppState>) -> Json<Vec<PipelineNode>> {
    Json(vec![
        PipelineNode {
            id: "inbound",
            name: "Inbound 端口监听",
            stage: "ingress",
            description: "UDP 53 / TCP 53 接收客户端请求并提取 Client IP",
            enabled: true,
        },
        PipelineNode {
            id: "qtype_drop",
            name: "Qtype 65 阻断",
            stage: "filter",
            description: "拦截苹果/Chrome HTTPS RR (Qtype 65) 避免绕过分流",
            enabled: true,
        },
        PipelineNode {
            id: "rewrite",
            name: "本地重写 & Hosts",
            stage: "resolve",
            description: "优先应答自定义 A/AAAA/CNAME 映射表",
            enabled: true,
        },
        PipelineNode {
            id: "adblock",
            name: "AdGuard 规则拦截",
            stage: "filter",
            description: "ABP/Hosts/正则语法毫秒级过滤广告与恶意域名",
            enabled: true,
        },
        PipelineNode {
            id: "cache",
            name: "SmartDNS 智能缓存",
            stage: "cache",
            description: "Serve-Expired 过期秒返与高频域名静默预取",
            enabled: true,
        },
        PipelineNode {
            id: "router",
            name: "MosDNS 分流器",
            stage: "routing",
            description: "国内 Direct 域名直连，Proxy 走境外，未知走 Fallback",
            enabled: true,
        },
        PipelineNode {
            id: "anti_pollution",
            name: "防 DNS 污染检测",
            stage: "security",
            description: "国内上游返回非大陆 IP 时丢弃应答并触发回退",
            enabled: state.engine.anti_pollution_enabled,
        },
        PipelineNode {
            id: "speed_check",
            name: "TCP 80/443 最优 IP 测速",
            stage: "optimize",
            description: "并发建连测速并按最低 RTT 重排序多条 IP 结果",
            enabled: true,
        },
    ])
}

async fn get_rules(State(state): State<AppState>) -> Result<Json<Vec<AdblockRuleRecord>>, (StatusCode, String)> {
    let rules = state
        .db
        .get_rules()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rules))
}

#[derive(Deserialize)]
struct AddRulePayload {
    name: String,
    url: String,
    content: Option<String>,
}

async fn add_rule(
    State(state): State<AppState>,
    Json(payload): Json<AddRulePayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = Uuid::new_v4().to_string();
    let mut rule_content = payload.content.unwrap_or_default();

    // If URL provided and content empty, try fetch
    if rule_content.is_empty() && payload.url.starts_with("http") {
        if let Ok(resp) = reqwest::get(&payload.url).await {
            if let Ok(text) = resp.text().await {
                rule_content = text;
            }
        }
    }

    let mut store = crate::pipeline::plugins::adblock::RuleStore::default();
    store.parse_rules(&rule_content);
    let count = store.total_count;

    let record = AdblockRuleRecord {
        id,
        name: payload.name,
        url: payload.url,
        enabled: true,
        rule_count: count,
        updated_at: Utc::now().to_rfc3339(),
    };

    state
        .db
        .save_rule(&record, &rule_content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Reload all rules into engine
    reload_all_rules(&state).await;

    Ok(StatusCode::CREATED)
}

async fn delete_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .delete_rule(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    reload_all_rules(&state).await;
    Ok(StatusCode::OK)
}

async fn refresh_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let rules = state
        .db
        .get_rules()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let Some(mut r) = rules.into_iter().find(|item| item.id == id) else {
        return Err((StatusCode::NOT_FOUND, "Rule not found".to_string()));
    };

    if r.url.starts_with("http") {
        let resp = reqwest::get(&r.url)
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;
        let text = resp
            .text()
            .await
            .map_err(|e| (StatusCode::BAD_GATEWAY, e.to_string()))?;

        let mut store = crate::pipeline::plugins::adblock::RuleStore::default();
        store.parse_rules(&text);
        r.rule_count = store.total_count;
        r.updated_at = Utc::now().to_rfc3339();

        state
            .db
            .save_rule(&r, &text)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        reload_all_rules(&state).await;
    }

    Ok(StatusCode::OK)
}

async fn reload_all_rules(state: &AppState) {
    if let Ok(rules) = state.db.get_rules() {
        let mut combined = String::new();
        let conn = match state.db.get_conn() {
            Ok(c) => c,
            Err(_) => return,
        };
        for r in rules {
            if r.enabled {
                if let Ok(content) = conn.query_row(
                    "SELECT content FROM adblock_rules WHERE id = ?1",
                    rusqlite::params![r.id],
                    |row| row.get::<_, Option<String>>(0),
                ) {
                    if let Some(c) = content {
                        combined.push_str(&c);
                        combined.push('\n');
                    }
                }
            }
        }
        state.engine.adblock.load_rules_from_text(&combined);
    }
}

#[derive(Deserialize)]
struct TestRulePayload {
    domain: String,
}

#[derive(Serialize)]
struct TestRuleResponse {
    domain: String,
    blocked: bool,
    matched_rule: Option<String>,
}

async fn test_rule(
    State(state): State<AppState>,
    Json(payload): Json<TestRulePayload>,
) -> Json<TestRuleResponse> {
    let mut ctx = crate::pipeline::context::QueryContext::new(
        "127.0.0.1".parse().unwrap(),
        hickory_proto::op::Message::query(),
    );
    ctx.qname = payload.domain.trim_end_matches('.').to_lowercase();
    let res = state.engine.adblock.execute(&mut ctx).await;
    let blocked = res == crate::pipeline::plugin::PluginResult::Accept;

    Json(TestRuleResponse {
        domain: payload.domain,
        blocked,
        matched_rule: ctx.matched_rule,
    })
}

#[derive(Serialize)]
struct UpstreamsResponse {
    mainland: Vec<String>,
    gfw: Vec<String>,
    overseas: Vec<String>,
}

async fn get_upstreams(State(state): State<AppState>) -> Json<UpstreamsResponse> {
    Json(UpstreamsResponse {
        mainland: state.config.upstreams.mainland.clone(),
        gfw: state.config.upstreams.gfw.clone(),
        overseas: state.config.upstreams.overseas.clone(),
    })
}

#[derive(Serialize)]
struct UpstreamTestItem {
    addr: String,
    group: String,
    latency_ms: Option<u64>,
    ok: bool,
}

async fn test_upstreams(State(state): State<AppState>) -> Json<Vec<UpstreamTestItem>> {
    let mut results = Vec::new();

    let groups = [
        ("mainland", &state.config.upstreams.mainland),
        ("gfw", &state.config.upstreams.gfw),
        ("overseas", &state.config.upstreams.overseas),
    ];

    for (group_name, list) in groups {
        for addr in list {
            let start = std::time::Instant::now();
            let server = crate::pipeline::plugins::upstream::UpstreamServer::parse("test", addr);
            let mut ok = false;
            let mut rtt = None;

            if let Some(s) = server {
                let mut req = hickory_proto::op::Message::query();
                req.add_query(hickory_proto::op::Query::query(
                    hickory_proto::rr::Name::from_ascii("example.com.").unwrap(),
                    hickory_proto::rr::RecordType::A,
                ));
                if let Ok(bytes) = req.to_vec() {
                    if s.query(&bytes, std::time::Duration::from_millis(1500)).await.is_ok() {
                        ok = true;
                        rtt = Some(start.elapsed().as_millis() as u64);
                    }
                }
            }

            results.push(UpstreamTestItem {
                addr: addr.clone(),
                group: group_name.to_string(),
                latency_ms: rtt,
                ok,
            });
        }
    }

    Json(results)
}

async fn get_rewrites(State(state): State<AppState>) -> Result<Json<Vec<DnsRewriteRecord>>, (StatusCode, String)> {
    let rewrites = state
        .db
        .get_rewrites()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(rewrites))
}

#[derive(Deserialize)]
struct AddRewritePayload {
    domain: String,
    record_type: String,
    value: String,
}

async fn add_rewrite(
    State(state): State<AppState>,
    Json(payload): Json<AddRewritePayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let id = Uuid::new_v4().to_string();
    let record = DnsRewriteRecord {
        id,
        domain: payload.domain.trim_end_matches('.').to_lowercase(),
        record_type: payload.record_type.to_uppercase(),
        value: payload.value,
        enabled: true,
    };

    state
        .db
        .save_rewrite(&record)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Reload rewrites into engine
    if let Ok(all) = state.db.get_rewrites() {
        let mut map = std::collections::HashMap::new();
        for r in all {
            if r.enabled {
                if let Ok(ip) = r.value.parse::<std::net::IpAddr>() {
                    map.entry(r.domain).or_insert_with(Vec::new).push(
                        crate::pipeline::plugins::rewrite::RewriteTarget::Ip(ip),
                    );
                } else {
                    map.entry(r.domain).or_insert_with(Vec::new).push(
                        crate::pipeline::plugins::rewrite::RewriteTarget::Cname(r.value),
                    );
                }
            }
        }
        state.engine.rewrite.set_records(map);
    }

    Ok(StatusCode::CREATED)
}

async fn delete_rewrite(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .delete_rewrite(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn flush_cache(State(state): State<AppState>) -> StatusCode {
    state.engine.cache.flush();
    StatusCode::OK
}

async fn ws_querylog_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws_querylog(socket, state))
}

async fn handle_ws_querylog(mut socket: WebSocket, state: AppState) {
    let mut rx = state.db.subscribe_ws();
    while let Ok(item) = rx.recv().await {
        if let Ok(json) = serde_json::to_string(&item) {
            if socket.send(WsMessage::Text(json.into())).await.is_err() {
                break;
            }
        }
    }
}
