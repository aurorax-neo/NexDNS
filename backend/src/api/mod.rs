use std::sync::Arc;
use std::sync::atomic::Ordering;
use axum::extract::ws::{Message as WsMessage, WebSocket, WebSocketUpgrade};
use axum::extract::{Path as AxumPath, Query, State, Request};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use rust_embed::RustEmbed;

use crate::config::AppConfig;
use crate::db::{auth::AuthManager, AdblockRuleRecord, Database, DnsRewriteRecord};
use crate::metrics::MetricsCollector;
use crate::pipeline::plugin::DnsPlugin;
use crate::pipeline::engine::PipelineEngine;
use crate::pipeline::rule_files::RuleFileManager;

#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct FrontendAssets;

#[derive(Clone)]
pub struct AppState {
    pub engine: Arc<PipelineEngine>,
    pub db: Arc<Database>,
    pub metrics: Arc<MetricsCollector>,
    pub config: Arc<parking_lot::RwLock<AppConfig>>,
    pub config_path: String,
    pub dns_server_enabled: Arc<parking_lot::RwLock<bool>>,
    pub rule_manager: Arc<RuleFileManager>,
    pub start_time: std::time::Instant,
}

impl AppState {
    pub fn reload_engine_upstreams(&self) {
        let (cfg, proxy) = {
            let c = self.config.read();
            (c.clone(), c.proxy.clone())
        };
        let mainland = cfg.upstreams.mainland.iter().filter_map(|u| {
            let mut s = crate::pipeline::plugins::upstream::UpstreamServer::parse("mainland", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "none".to_string());
            s.proxy = proxy.find_proxy_addr(&proxy_mode);
            Some(s)
        }).collect();
        let gfw = cfg.upstreams.gfw.iter().filter_map(|u| {
            let mut s = crate::pipeline::plugins::upstream::UpstreamServer::parse("gfw", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "default".to_string());
            s.proxy = proxy.find_proxy_addr(&proxy_mode);
            Some(s)
        }).collect();
        let overseas = cfg.upstreams.overseas.iter().filter_map(|u| {
            let mut s = crate::pipeline::plugins::upstream::UpstreamServer::parse("overseas", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "default".to_string());
            s.proxy = proxy.find_proxy_addr(&proxy_mode);
            Some(s)
        }).collect();
        self.engine.reload_upstreams(mainland, gfw, overseas);
    }
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/dns-query", get(doh_handler).post(doh_handler))
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/logout", post(logout_handler))
        .route("/api/v1/auth/me", get(auth_me_handler))
        .route("/api/v1/auth/password", post(change_password_handler))
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/settings", get(get_settings).put(update_settings))
        .route("/api/v1/proxy/test", post(test_proxy))
        .route("/api/v1/querylog", get(get_query_logs))
        .route("/api/v1/querylog/export", get(export_query_logs_csv))
        .route("/api/v1/querylog/clear", post(clear_query_logs))
        .route("/api/v1/pipeline", get(get_pipeline))
        .route("/api/v1/pipeline/toggle", post(toggle_pipeline_stage))
        .route("/api/v1/rules", get(get_rules).post(add_rule))
        .route("/api/v1/rules/{id}", delete(delete_rule))
        .route("/api/v1/rules/{id}/refresh", post(refresh_rule))
        .route("/api/v1/rules/{id}/toggle", post(toggle_rule))
        .route("/api/v1/rules/{id}/content", get(get_rule_content).put(update_rule_content))
        .route("/api/v1/rules/test", post(test_rule))
        .route("/api/v1/rules/quick-add", post(quick_add_rule))
        .route("/api/v1/upstreams", get(get_upstreams))
        .route("/api/v1/upstreams/test", post(test_upstreams))
        .route("/api/v1/upstreams/test-single", post(test_single_upstream))
        .route("/api/v1/upstreams/add", post(add_upstream))
        .route("/api/v1/upstreams/edit", post(edit_upstream))
        .route("/api/v1/upstreams/reorder", post(reorder_upstreams))
        .route("/api/v1/upstreams/remove", post(remove_upstream))
        .route("/api/v1/routing/rules", get(get_routing_rules))
        .route("/api/v1/routing/rules/update", post(update_routing_rules))
        .route("/api/v1/routing/rules/add", post(add_routing_rule))
        .route("/api/v1/routing/rules/{id}", delete(delete_routing_rule))
        .route("/api/v1/routing/rules/{id}/edit", post(edit_routing_rule))
        .route("/api/v1/routing/rules/{id}/toggle", post(toggle_routing_rule))
        .route("/api/v1/routing/rules/{id}/refresh", post(refresh_routing_rule))
        .route("/api/v1/routing/rules/{file_name}/content", get(get_routing_rule_content).put(update_routing_rule_content))
        .route("/api/v1/config/raw", get(get_raw_config).put(update_raw_config))
        .route("/api/v1/rewrites", get(get_rewrites).post(add_rewrite))
        .route("/api/v1/rewrites/{id}", delete(delete_rewrite))
        .route("/api/v1/cache/flush", post(flush_cache))
        .route("/api/v1/ws/querylog", get(ws_querylog_handler))
        .fallback(static_handler)
        .with_state(state)
}

#[derive(Deserialize)]
struct LoginPayload {
    username: String,
    password: String,
}

#[derive(Serialize)]
struct LoginResponse {
    token: String,
    username: String,
    expires_at: String,
}

async fn login_handler(
    State(state): State<AppState>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match AuthManager::authenticate(&conn, &payload.username, &payload.password) {
        Ok(Some((token, expires_at))) => Ok(Json(LoginResponse {
            token,
            username: payload.username,
            expires_at,
        })),
        Ok(None) => Err((StatusCode::UNAUTHORIZED, "用户名或密码错误".to_string())),
        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

#[derive(Deserialize)]
struct LogoutPayload {
    token: String,
}

async fn logout_handler(
    State(state): State<AppState>,
    Json(payload): Json<LogoutPayload>,
) -> StatusCode {
    if let Ok(conn) = state.db.get_conn() {
        let _ = AuthManager::logout(&conn, &payload.token);
    }
    StatusCode::OK
}

#[derive(Serialize)]
struct AuthMeResponse {
    username: String,
    authenticated: bool,
}

async fn auth_me_handler(
    State(state): State<AppState>,
    request: Request,
) -> Json<AuthMeResponse> {
    let token = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    if let Some(t) = token {
        if let Ok(conn) = state.db.get_conn() {
            if let Some(username) = AuthManager::validate_token(&conn, t) {
                return Json(AuthMeResponse { username, authenticated: true });
            }
        }
    }

    Json(AuthMeResponse {
        username: String::new(),
        authenticated: false,
    })
}

#[derive(Deserialize)]
struct ChangePasswordPayload {
    username: String,
    old_password: String,
    new_password: String,
}

async fn change_password_handler(
    State(state): State<AppState>,
    Json(payload): Json<ChangePasswordPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    AuthManager::change_password(&conn, &payload.username, &payload.old_password, &payload.new_password)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    Ok(StatusCode::OK)
}

async fn get_settings(State(state): State<AppState>) -> Json<AppConfig> {
    Json(state.config.read().clone())
}

async fn update_settings(
    State(state): State<AppState>,
    Json(payload): Json<AppConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    // Hot apply to engine
    state.engine.anti_pollution_enabled.store(payload.anti_pollution, Ordering::SeqCst);
    state.engine.qtype_65_enabled.store(payload.routing.qtype_65_reject, Ordering::SeqCst);
    state.engine.speed_check_enabled.store(payload.speed_check.enabled, Ordering::SeqCst);
    state.engine.cache.min_ttl.store(payload.cache.min_ttl, Ordering::SeqCst);
    state.engine.cache.max_ttl.store(payload.cache.max_ttl, Ordering::SeqCst);
    *state.engine.response_mode.write() = payload.dns.response_mode.clone();
    *state.dns_server_enabled.write() = payload.dns.enabled;

    *state.config.write() = payload.clone();
    payload.save(&state.config_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.reload_engine_upstreams();

    Ok(StatusCode::OK)
}

pub async fn doh_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: axum::body::Bytes,
) -> axum::response::Response {
    let doh_enabled = state.config.read().dns.doh_enabled;

    if !doh_enabled {
        return (
            StatusCode::FORBIDDEN,
            [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "DoH service is disabled",
        ).into_response();
    }

    let req_bytes = if !body.is_empty() {
        body.to_vec()
    } else if let Some(dns_b64) = params.get("dns") {
        use data_encoding::BASE64URL_NOPAD;
        match BASE64URL_NOPAD.decode(dns_b64.as_bytes()) {
            Ok(b) => b,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
                    "Invalid base64url dns parameter",
                ).into_response();
            }
        }
    } else {
        return (
            StatusCode::BAD_REQUEST,
            [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Missing DNS request body or ?dns parameter",
        ).into_response();
    };

    let Ok(msg) = hickory_proto::op::Message::from_vec(&req_bytes) else {
        return (
            StatusCode::BAD_REQUEST,
            [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Invalid DNS wire format",
        ).into_response();
    };

    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok())
        .unwrap_or_else(|| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)));

    let mut ctx = crate::pipeline::context::QueryContext::new(client_ip, msg);
    state.engine.process(&mut ctx).await;

    state.db.record_query(&ctx);
    state.metrics.record(&ctx);

    if let Some(resp) = ctx.response {
        if let Ok(bytes) = resp.to_vec() {
            let mut resp_headers = axum::http::HeaderMap::new();
            resp_headers.insert(axum::http::header::CONTENT_TYPE, "application/dns-message".parse().unwrap());
            resp_headers.insert(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
            return (
                StatusCode::OK,
                resp_headers,
                bytes,
            ).into_response();
        }
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        "Failed to generate DNS response".as_bytes().to_vec(),
    ).into_response()
}

#[derive(Deserialize)]
struct TestProxyPayload {
    socks5: String,
}

#[derive(Serialize)]
struct TestProxyResponse {
    ok: bool,
    latency_ms: Option<f64>,
    error: Option<String>,
}

async fn test_proxy(
    Json(payload): Json<TestProxyPayload>,
) -> Json<TestProxyResponse> {
    let start = std::time::Instant::now();
    let mut proxy_str = payload.socks5.trim().to_string();
    if proxy_str.is_empty() {
        return Json(TestProxyResponse {
            ok: false,
            latency_ms: None,
            error: Some("代理地址不能为空".to_string()),
        });
    }

    if !proxy_str.contains("://") {
        proxy_str = format!("socks5://{}", proxy_str);
    }

    match reqwest::Proxy::all(&proxy_str) {
        Ok(p) => {
            match reqwest::Client::builder()
                .proxy(p)
                .timeout(std::time::Duration::from_millis(4000))
                .build()
            {
                Ok(client) => {
                    let test_urls = [
                        "https://dns.google/resolve?name=example.com",
                        "https://www.google.com/generate_204",
                        "https://1.1.1.1/dns-query",
                        "https://cloudflare-dns.com/dns-query",
                    ];
                    let mut last_err = String::new();
                    for url in test_urls {
                        match client.get(url).send().await {
                            Ok(_) => {
                                let rtt = (start.elapsed().as_micros() as f64) / 1000.0;
                                return Json(TestProxyResponse {
                                    ok: true,
                                    latency_ms: Some(rtt),
                                    error: None,
                                });
                            }
                            Err(e) => {
                                last_err = e.to_string();
                            }
                        }
                    }
                    Json(TestProxyResponse {
                        ok: false,
                        latency_ms: None,
                        error: Some(format!("代理连接失败: {}", last_err)),
                    })
                }
                Err(e) => Json(TestProxyResponse {
                    ok: false,
                    latency_ms: None,
                    error: Some(e.to_string()),
                }),
            }
        }
        Err(e) => Json(TestProxyResponse {
            ok: false,
            latency_ms: None,
            error: Some(format!("代理配置解析失败: {}", e)),
        }),
    }
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
    let (dns_p, web_p) = {
        let c = state.config.read();
        (c.dns.port, c.web.port)
    };
    Json(StatusResponse {
        version: "0.1.0",
        uptime_seconds: state.start_time.elapsed().as_secs(),
        dns_port: dns_p,
        web_port: web_p,
        cache_entries: state.engine.cache.size(),
        adblock_rules_count: state.engine.adblock.rule_count(),
        anti_pollution: state.engine.anti_pollution_enabled.load(Ordering::Relaxed),
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
    let logging_enabled = state.config.read().logging.enabled;
    if !logging_enabled {
        return Ok(Json(QueryLogResponse {
            items: vec![],
            total: 0,
            page: 1,
            page_size: params.page_size.unwrap_or(30),
        }));
    }

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
            enabled: state.engine.qtype_65_enabled.load(Ordering::Relaxed),
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
            name: "安全规则拦截",
            stage: "filter",
            description: "标准规则/Hosts/正则高并发毫秒级过滤广告与威胁域名",
            enabled: state.engine.adblock_enabled.load(Ordering::Relaxed),
        },
        PipelineNode {
            id: "cache",
            name: "高性能智能缓存",
            stage: "cache",
            description: "Serve-Expired 零等待秒级应答与热点记录静默预取",
            enabled: state.engine.cache_enabled.load(Ordering::Relaxed),
        },
        PipelineNode {
            id: "router",
            name: "动态分流调度引擎",
            stage: "routing",
            description: "按 Direct 直连、Proxy 代理及 GeoIP 归属进行原生动态分流与防污染调度",
            enabled: true,
        },
        PipelineNode {
            id: "anti_pollution",
            name: "防 DNS 污染检测",
            stage: "security",
            description: "国内上游返回非大陆 IP 时丢弃应答并触发回退",
            enabled: state.engine.anti_pollution_enabled.load(Ordering::Relaxed),
        },
        PipelineNode {
            id: "speed_check",
            name: "TCP 80/443 最优 IP 测速",
            stage: "optimize",
            description: "并发建连测速并按最低 RTT 重排序多条 IP 结果",
            enabled: state.engine.speed_check_enabled.load(Ordering::Relaxed),
        },
    ])
}

#[derive(Deserialize)]
struct TogglePipelinePayload {
    id: String,
    enabled: bool,
}

async fn toggle_pipeline_stage(
    State(state): State<AppState>,
    Json(payload): Json<TogglePipelinePayload>,
) -> StatusCode {
    if state.engine.toggle_stage(&payload.id, payload.enabled) {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    }
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

    // If URL provided and content empty, try fetch with 6s timeout, fail storage as empty file
    if rule_content.is_empty() && payload.url.starts_with("http") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()
            .unwrap_or_default();
        if let Ok(resp) = client.get(&payload.url).send().await {
            if let Ok(text) = resp.text().await {
                rule_content = text;
            }
        }
    }

    let mut store = crate::pipeline::plugins::adblock::RuleStore::default();
    store.parse_rules(&rule_content);
    let count = store.total_count;

    // 智能规则源重名处理: 若已有同名规则源，自动附加序号避免混淆
    let mut safe_name = payload.name.trim().to_string();
    if let Ok(existing_rules) = state.db.get_rules() {
        let mut idx = 1;
        while existing_rules.iter().any(|r| r.name == safe_name) {
            safe_name = format!("{} ({})", payload.name.trim(), idx);
            idx += 1;
        }
    }

    let record = AdblockRuleRecord {
        id,
        name: safe_name,
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
        return Err((StatusCode::NOT_FOUND, "未找到指定的规则源".to_string()));
    };

    if r.url.is_empty() || !r.url.starts_with("http") {
        return Err((StatusCode::BAD_REQUEST, "该规则源未配置在线订阅 URL，无法进行在线刷新".to_string()));
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let resp = client.get(&r.url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("拉取在线订阅失败: {}", e)))?;
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

    Ok(StatusCode::OK)
}

pub async fn reload_all_rules(state: &AppState) {
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
    let cfg = state.config.read();
    Json(UpstreamsResponse {
        mainland: cfg.upstreams.mainland.clone(),
        gfw: cfg.upstreams.gfw.clone(),
        overseas: cfg.upstreams.overseas.clone(),
    })
}

#[derive(Serialize)]
struct UpstreamTestItem {
    addr: String,
    group: String,
    latency_ms: Option<f64>,
    ok: bool,
}

async fn test_upstreams(State(state): State<AppState>) -> Json<Vec<UpstreamTestItem>> {
    let (mainland, gfw, overseas, proxy_cfg) = {
        let cfg = state.config.read();
        (cfg.upstreams.mainland.clone(), cfg.upstreams.gfw.clone(), cfg.upstreams.overseas.clone(), cfg.proxy.clone())
    };
    let mut results = Vec::new();

    let groups = [
        ("mainland", &mainland),
        ("gfw", &gfw),
        ("overseas", &overseas),
    ];

    for (group_name, list) in groups {
        for addr in list {
            let start = std::time::Instant::now();
            let server = crate::pipeline::plugins::upstream::UpstreamServer::parse("test", addr);
            let mut ok = false;
            let mut rtt = None;

            if let Some(mut s) = server {
                let proxy_mode = s.proxy.clone().unwrap_or_else(|| {
                    if group_name == "mainland" { "none".to_string() } else { "default".to_string() }
                });
                s.proxy = proxy_cfg.find_proxy_addr(&proxy_mode);
                let mut req = hickory_proto::op::Message::query();
                req.add_query(hickory_proto::op::Query::query(
                    hickory_proto::rr::Name::from_ascii("example.com.").unwrap(),
                    hickory_proto::rr::RecordType::A,
                ));
                if let Ok(bytes) = req.to_vec() {
                    if s.query(&bytes, std::time::Duration::from_millis(3500)).await.is_ok() {
                        ok = true;
                        rtt = Some((start.elapsed().as_micros() as f64) / 1000.0);
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
async fn toggle_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    conn.execute(
        "UPDATE adblock_rules SET enabled = (CASE WHEN enabled = 1 THEN 0 ELSE 1 END) WHERE id = ?1",
        rusqlite::params![id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    reload_all_rules(&state).await;
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct QuickAddRulePayload {
    domain: String,
    action: String, // "block" or "whitelist"
}

async fn quick_add_rule(
    State(state): State<AppState>,
    Json(payload): Json<QuickAddRulePayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let domain = payload.domain.trim_end_matches('.').to_lowercase();
    if domain.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "Domain empty".to_string()));
    }

    let line = if payload.action == "whitelist" {
        format!("@@||{}^\n", domain)
    } else {
        format!("||{}^\n", domain)
    };

    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Find or create "自定义规则" list
    let existing_custom: Option<String> = conn.query_row(
        "SELECT id FROM adblock_rules WHERE name = '自定义规则' LIMIT 1",
        [],
        |r| r.get(0),
    ).ok();

    let rule_id = existing_custom.unwrap_or_else(|| {
        let new_id = Uuid::new_v4().to_string();
        let _ = conn.execute(
            "INSERT INTO adblock_rules (id, name, url, enabled, rule_count, updated_at, content)
             VALUES (?1, '自定义规则', 'custom-local', 1, 0, ?2, '')",
            rusqlite::params![new_id, Utc::now().to_rfc3339()],
        );
        new_id
    });

    conn.execute(
        "UPDATE adblock_rules SET content = coalesce(content, '') || ?1, updated_at = ?2, rule_count = rule_count + 1 WHERE id = ?3",
        rusqlite::params![line, Utc::now().to_rfc3339(), rule_id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    reload_all_rules(&state).await;
    Ok(StatusCode::OK)
}
#[derive(Deserialize)]
struct UpstreamActionPayload {
    group: String,
    addr: String,
}

async fn add_upstream(
    State(state): State<AppState>,
    Json(payload): Json<UpstreamActionPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let mut cfg = state.config.write();
        match payload.group.as_str() {
            "mainland" => cfg.upstreams.mainland.push(payload.addr),
            "gfw" => cfg.upstreams.gfw.push(payload.addr),
            "overseas" => cfg.upstreams.overseas.push(payload.addr),
            _ => return Err((StatusCode::BAD_REQUEST, "Unknown group".to_string())),
        }
        let _ = cfg.save(&state.config_path);
    }
    state.reload_engine_upstreams();
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct SingleUpstreamTestPayload {
    group: String,
    addr: String,
}

#[derive(Serialize)]
struct SingleUpstreamTestResponse {
    ok: bool,
    latency_ms: Option<f64>,
    error: Option<String>,
}

async fn test_single_upstream(
    State(state): State<AppState>,
    Json(payload): Json<SingleUpstreamTestPayload>,
) -> Json<SingleUpstreamTestResponse> {
    let start = std::time::Instant::now();
    let server = crate::pipeline::plugins::upstream::UpstreamServer::parse(&payload.group, &payload.addr);
    let proxy_cfg = state.config.read().proxy.clone();

    if let Some(mut s) = server {
        let proxy_mode = s.proxy.clone().unwrap_or_else(|| {
            if payload.group == "mainland" { "none".to_string() } else { "default".to_string() }
        });
        s.proxy = proxy_cfg.find_proxy_addr(&proxy_mode);

        let mut req = hickory_proto::op::Message::query();
        req.add_query(hickory_proto::op::Query::query(
            hickory_proto::rr::Name::from_ascii("example.com.").unwrap(),
            hickory_proto::rr::RecordType::A,
        ));

        if let Ok(bytes) = req.to_vec() {
            match s.query(&bytes, std::time::Duration::from_millis(3500)).await {
                Ok(_) => {
                    let rtt = (start.elapsed().as_micros() as f64) / 1000.0;
                    return Json(SingleUpstreamTestResponse {
                        ok: true,
                        latency_ms: Some(rtt),
                        error: None,
                    });
                }
                Err(e) => {
                    return Json(SingleUpstreamTestResponse {
                        ok: false,
                        latency_ms: None,
                        error: Some(e),
                    });
                }
            }
        }
    }

    Json(SingleUpstreamTestResponse {
        ok: false,
        latency_ms: None,
        error: Some("解析或连接上游失败".to_string()),
    })
}

#[derive(Deserialize)]
struct EditUpstreamPayload {
    old_group: String,
    old_addr: String,
    new_group: String,
    new_addr: String,
}

async fn edit_upstream(
    State(state): State<AppState>,
    Json(payload): Json<EditUpstreamPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let mut cfg = state.config.write();
        match payload.old_group.as_str() {
            "mainland" => cfg.upstreams.mainland.retain(|s| s != &payload.old_addr),
            "gfw" => cfg.upstreams.gfw.retain(|s| s != &payload.old_addr),
            "overseas" => cfg.upstreams.overseas.retain(|s| s != &payload.old_addr),
            _ => return Err((StatusCode::BAD_REQUEST, "Unknown old group".to_string())),
        }
        match payload.new_group.as_str() {
            "mainland" => cfg.upstreams.mainland.push(payload.new_addr),
            "gfw" => cfg.upstreams.gfw.push(payload.new_addr),
            "overseas" => cfg.upstreams.overseas.push(payload.new_addr),
            _ => return Err((StatusCode::BAD_REQUEST, "Unknown new group".to_string())),
        }
        let _ = cfg.save(&state.config_path);
    }
    state.reload_engine_upstreams();
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
struct ReorderUpstreamsPayload {
    group: String,
    addrs: Vec<String>,
}

async fn reorder_upstreams(
    State(state): State<AppState>,
    Json(payload): Json<ReorderUpstreamsPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let mut cfg = state.config.write();
        match payload.group.as_str() {
            "mainland" => cfg.upstreams.mainland = payload.addrs,
            "gfw" => cfg.upstreams.gfw = payload.addrs,
            "overseas" => cfg.upstreams.overseas = payload.addrs,
            _ => return Err((StatusCode::BAD_REQUEST, "Unknown group".to_string())),
        }
        let _ = cfg.save(&state.config_path);
    }
    state.reload_engine_upstreams();
    Ok(StatusCode::OK)
}

async fn remove_upstream(
    State(state): State<AppState>,
    Json(payload): Json<UpstreamActionPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    {
        let mut cfg = state.config.write();
        match payload.group.as_str() {
            "mainland" => cfg.upstreams.mainland.retain(|s| s != &payload.addr),
            "gfw" => cfg.upstreams.gfw.retain(|s| s != &payload.addr),
            "overseas" => cfg.upstreams.overseas.retain(|s| s != &payload.addr),
            _ => return Err((StatusCode::BAD_REQUEST, "Unknown group".to_string())),
        }
        let _ = cfg.save(&state.config_path);
    }
    state.reload_engine_upstreams();
    Ok(StatusCode::OK)
}

async fn get_routing_rules(
    State(state): State<AppState>,
) -> Json<Vec<crate::pipeline::rule_files::RuleFileInfo>> {
    Json(state.rule_manager.list_rules())
}

async fn update_routing_rules(
    State(state): State<AppState>,
) -> Json<Vec<(String, Result<usize, String>)>> {
    let res = state.rule_manager.update_all().await;
    Json(res)
}

#[derive(Serialize)]
struct RuleContentResponse {
    id: String,
    content: String,
}

#[derive(Deserialize)]
struct UpdateRuleContentPayload {
    content: String,
}

async fn get_rule_content(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RuleContentResponse>, (StatusCode, String)> {
    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let content: String = conn.query_row(
        "SELECT coalesce(content, '') FROM adblock_rules WHERE id = ?1",
        rusqlite::params![id],
        |r| r.get(0),
    ).map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;

    Ok(Json(RuleContentResponse { id, content }))
}

async fn update_rule_content(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<UpdateRuleContentPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let mut store = crate::pipeline::plugins::adblock::RuleStore::default();
    store.parse_rules(&payload.content);
    let count = store.total_count;

    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    conn.execute(
        "UPDATE adblock_rules SET content = ?1, rule_count = ?2, updated_at = ?3 WHERE id = ?4",
        rusqlite::params![payload.content, count as i64, Utc::now().to_rfc3339(), id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    reload_all_rules(&state).await;
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct RoutingRuleContentResponse {
    file_name: String,
    content: String,
}

async fn get_routing_rule_content(
    State(_state): State<AppState>,
    AxumPath(file_name): AxumPath<String>,
) -> Result<Json<RoutingRuleContentResponse>, (StatusCode, String)> {
    let path = std::path::Path::new("data/rules").join(&file_name);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(Json(RoutingRuleContentResponse { file_name, content }))
}

async fn update_routing_rule_content(
    State(state): State<AppState>,
    AxumPath(file_name): AxumPath<String>,
    Json(payload): Json<UpdateRuleContentPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let path = std::path::Path::new("data/rules").join(&file_name);
    std::fs::write(&path, &payload.content).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.rule_manager.load_local_rules();
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct RawConfigResponse {
    content: String,
}

async fn get_raw_config() -> Result<Json<RawConfigResponse>, (StatusCode, String)> {
    let content = std::fs::read_to_string("data/config.yaml").unwrap_or_default();
    Ok(Json(RawConfigResponse { content }))
}

async fn update_raw_config(
    Json(payload): Json<UpdateRuleContentPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let _: crate::config::AppConfig = serde_yaml::from_str(&payload.content)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid YAML: {}", e)))?;
    std::fs::write("data/config.yaml", &payload.content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}
#[derive(Deserialize)]
struct AddRoutingRulePayload {
    name: String,
    file_name: String,
    group_type: String,
    url: Option<String>,
    content: Option<String>,
}

async fn add_routing_rule(
    State(state): State<AppState>,
    Json(payload): Json<AddRoutingRulePayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let url_str = payload.url.unwrap_or_default();
    let mut content_str = payload.content.unwrap_or_default();

    if content_str.is_empty() && url_str.starts_with("http") {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(6))
            .build()
            .unwrap_or_default();
        if let Ok(resp) = client.get(&url_str).send().await {
            if let Ok(text) = resp.text().await {
                content_str = text;
            }
        }
    }

    state.rule_manager.add_rule_file(
        &payload.name,
        &payload.file_name,
        &payload.group_type,
        &url_str,
        &content_str,
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(StatusCode::CREATED)
}

#[derive(Deserialize)]
struct EditRoutingRulePayload {
    name: String,
    group_type: String,
    url: String,
}

async fn edit_routing_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<EditRoutingRulePayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.rule_manager.edit_rule_info(&id, &payload.name, &payload.group_type, &payload.url)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn delete_routing_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let rules = state.rule_manager.list_rules();
    if let Some(r) = rules.iter().find(|item| item.id == id) {
        const CORE_FILES: &[&str] = &["direct.txt", "proxy.txt", "private.txt", "ipv4_china.txt", "ipv6_china.txt"];
        if CORE_FILES.contains(&r.file_name.as_str()) {
            return Err((StatusCode::FORBIDDEN, "系统核心分流规则文件禁止删除".to_string()));
        }
    }
    state.rule_manager.delete_rule_file(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn toggle_routing_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state.rule_manager.toggle_rule_file(&id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::OK)
}

async fn refresh_routing_rule(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    let rules = state.rule_manager.list_rules();
    let Some(r) = rules.into_iter().find(|item| item.id == id) else {
        return Err((StatusCode::NOT_FOUND, "未找到指定的规则文件".to_string()));
    };

    if r.url.is_empty() || !r.url.starts_with("http") {
        return Err((StatusCode::BAD_REQUEST, "该规则文件未配置在线订阅 URL，无法进行在线刷新".to_string()));
    }

    state.rule_manager.update_rule_file(&r.file_name, &r.url).await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("拉取在线规则失败: {}", e)))?;

    Ok(StatusCode::OK)
}
