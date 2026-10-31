use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::net::SocketAddr;
use axum::extract::{ConnectInfo, Path as AxumPath, Query, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Local;
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
    pub logging_server_enabled: Arc<parking_lot::RwLock<bool>>,
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
            s.set_proxy(proxy.find_proxy_addr(&proxy_mode));
            Some(s)
        }).collect();
        let gfw = cfg.upstreams.gfw.iter().filter_map(|u| {
            let mut s = crate::pipeline::plugins::upstream::UpstreamServer::parse("gfw", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "default".to_string());
            s.set_proxy(proxy.find_proxy_addr(&proxy_mode));
            Some(s)
        }).collect();
        let overseas = cfg.upstreams.overseas.iter().filter_map(|u| {
            let mut s = crate::pipeline::plugins::upstream::UpstreamServer::parse("overseas", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "default".to_string());
            s.set_proxy(proxy.find_proxy_addr(&proxy_mode));
            Some(s)
        }).collect();
        self.engine.reload_upstreams(mainland, gfw, overseas);
    }
}

pub fn create_web_router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/logout", post(logout_handler))
        .route("/api/v1/auth/me", get(auth_me_handler))
        .route("/api/v1/auth/password", post(change_password_handler))
        .route("/api/v1/status", get(get_status))
        .route("/api/v1/metrics", get(get_metrics))
        .route("/api/v1/metrics/cache-stats", get(get_cache_stats))
        .route("/api/v1/metrics/cache-stats/clear", post(clear_cache_stats))
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
        .route("/api/v1/upstreams/test-group", post(test_group_upstreams))
        .route("/api/v1/upstreams/test-single", post(test_single_upstream))
        .route("/api/v1/upstreams/add", post(add_upstream))
        .route("/api/v1/upstreams/edit", post(edit_upstream))
        .route("/api/v1/upstreams/reorder", post(reorder_upstreams))
        .route("/api/v1/upstreams/remove", post(remove_upstream))
        .route("/api/v1/upstreams/bootstrap", post(update_bootstrap_dns))
        .route("/api/v1/routing/rules", get(get_routing_rules))
        .route("/api/v1/routing/rules/update", post(update_routing_rules))
        .route("/api/v1/routing/rules/add", post(add_routing_rule))
        .route("/api/v1/routing/rules/{id}", delete(delete_routing_rule))
        .route("/api/v1/routing/rules/{id}/edit", post(edit_routing_rule))
        .route("/api/v1/routing/rules/{id}/toggle", post(toggle_routing_rule))
        .route("/api/v1/routing/rules/{id}/refresh", post(refresh_routing_rule))
        .route("/api/v1/routing/rules/{file_name}/content", get(get_routing_rule_content).put(update_routing_rule_content))
        .route("/api/v1/tools/query", post(tool_query_dns))
        .route("/api/v1/config/raw", get(get_raw_config).put(update_raw_config))
        .route("/api/v1/rewrites", get(get_rewrites).post(add_rewrite))
        .route("/api/v1/rewrites/{id}", delete(delete_rewrite))
        .route("/api/v1/cache/flush", post(flush_cache))
        .route("/api/v1/sse/querylog", get(sse_querylog_handler))
        .fallback(static_handler)
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state)
}

pub fn create_doh_router(state: AppState) -> Router {
    let doh_path = {
        let c = state.config.read();
        let p = c.dns.doh_path.trim().to_string();
        if p.is_empty() {
            "/dns-query".to_string()
        } else if !p.starts_with('/') {
            format!("/{}", p)
        } else {
            p
        }
    };

    let mut router = Router::new()
        .route("/dns-query", get(doh_handler).post(doh_handler));

    if doh_path != "/dns-query" {
        router = router.route(&doh_path, get(doh_handler).post(doh_handler));
    }

    router
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(state)
}

#[allow(dead_code)]
pub fn create_router(state: AppState) -> Router {
    create_web_router(state)
}
async fn auth_middleware(
    State(state): State<AppState>,
    req: Request,
    next: axum::middleware::Next,
) -> Result<Response, (StatusCode, &'static str)> {
    let path = req.uri().path();
    // 允许登录与检查登录状态、前端静态资源免认证
    // 注意: /dns-query 已完全移至独立 DoH 端口，Web 控制台端口不再响应 DoH 请求
    if path == "/api/v1/auth/login"
        || path == "/api/v1/auth/me"
        || !path.starts_with("/api/v1/")
    {
        return Ok(next.run(req).await);
    }

    // 从请求头 Authorization: Bearer <token> 或 Query 参数 ?token=<token> 中提取凭据
    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .or_else(|| {
            req.uri().query().and_then(|q| {
                for pair in q.split('&') {
                    if let Some((k, v)) = pair.split_once('=') {
                        if k == "token" {
                            return Some(v.to_string());
                        }
                    }
                }
                None
            })
        });

    if let Some(t) = token {
        if let Ok(conn) = state.db.get_conn() {
            if AuthManager::validate_token(&conn, &t).is_some() {
                return Ok(next.run(req).await);
            }
        }
    }

    Err((StatusCode::UNAUTHORIZED, "Unauthorized"))
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
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<LoginPayload>,
) -> Result<Json<LoginResponse>, (StatusCode, String)> {
    let client_ip = peer_addr.ip().to_string();
    if let Err(e) = AuthManager::check_rate_limit(&client_ip) {
        return Err((StatusCode::TOO_MANY_REQUESTS, e));
    }
    let conn = state.db.get_conn().map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    match AuthManager::authenticate(&conn, &payload.username, &payload.password) {
        Ok(Some((token, expires_at))) => {
            AuthManager::reset_login_attempts(&client_ip);
            Ok(Json(LoginResponse {
                token,
                username: payload.username,
                expires_at,
            }))
        }
        Ok(None) => {
            AuthManager::record_login_failure(&client_ip);
            Err((StatusCode::UNAUTHORIZED, "用户名或密码错误".to_string()))
        }
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

fn validate_config_safety(payload: &AppConfig) -> Result<(), (StatusCode, String)> {
    if payload.dns.port == 0 {
        return Err((StatusCode::BAD_REQUEST, "DNS 监听端口不能为 0".to_string()));
    }
    if payload.web.port == 0 {
        return Err((StatusCode::BAD_REQUEST, "Web 控制台端口不能为 0".to_string()));
    }
    if payload.dns.doh_enabled && payload.dns.doh_port > 0 && payload.dns.doh_port == payload.web.port {
        return Err((StatusCode::BAD_REQUEST, "DoH 独立端口禁止与 Web 控制台端口冲突".to_string()));
    }
    if payload.cache.min_ttl > payload.cache.max_ttl {
        return Err((StatusCode::BAD_REQUEST, "缓存最小 TTL 不能大于最大 TTL".to_string()));
    }
    for b in &payload.upstreams.bootstrap_dns {
        if let Err(e) = validate_bootstrap_addr(b) {
            return Err((StatusCode::BAD_REQUEST, e));
        }
    }
    if payload.routing.enabled {
        let gfw_has_server = payload.upstreams.gfw.iter().any(|s| !s.trim().is_empty());
        let overseas_has_server = payload.upstreams.overseas.iter().any(|s| !s.trim().is_empty());
        if !gfw_has_server || !overseas_has_server {
            return Err((
                StatusCode::BAD_REQUEST,
                "无法开启动态分流调度引擎：GFW / 境外组 (gfw) 与海外备用组 (overseas) 必须各自至少配置一个上游 DNS 服务器".to_string(),
            ));
        }
    }
    Ok(())
}

fn apply_config_to_runtime(state: &AppState, payload: &AppConfig) {
    state.engine.anti_pollution_enabled.store(payload.anti_pollution, Ordering::SeqCst);
    state.engine.router_enabled.store(payload.routing.enabled, Ordering::SeqCst);
    state.engine.speed_check_enabled.store(payload.speed_check.enabled, Ordering::SeqCst);
    state.engine.speed_check.set_enabled(payload.speed_check.enabled);
    state.engine.cache.min_ttl.store(payload.cache.min_ttl, Ordering::SeqCst);
    state.engine.cache.max_ttl.store(payload.cache.max_ttl, Ordering::SeqCst);
    state.engine.cache.prefetch.store(payload.cache.prefetch, Ordering::SeqCst);
    let bt = crate::pipeline::plugins::adblock::BlockType::from_str_name(&payload.dns.block_type);
    state.engine.adblock.set_block_type(bt);
    state.engine.adblock.set_block_ttl(payload.dns.block_ttl);
    *state.engine.response_mode.write() = payload.dns.response_mode.clone();
    *state.dns_server_enabled.write() = payload.dns.enabled;
    *state.logging_server_enabled.write() = payload.logging.enabled;

    *state.config.write() = payload.clone();
    state.reload_engine_upstreams();
}

async fn update_settings(
    State(state): State<AppState>,
    Json(payload): Json<AppConfig>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_config_safety(&payload)?;
    payload.save(&state.config_path).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    apply_config_to_runtime(&state, &payload);
    Ok(StatusCode::OK)
}

pub async fn doh_handler(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
    body: axum::body::Bytes,
) -> axum::response::Response {
    let dns_enabled = *state.dns_server_enabled.read();
    if !dns_enabled {
        return (
            StatusCode::FORBIDDEN,
            [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "DNS service is disabled",
        ).into_response();
    }

    let req_bytes = if !body.is_empty() {
        body.to_vec()
    } else if let Some(dns_b64) = params.get("dns") {
        let clean_b64 = dns_b64.trim();
        let unpadded = clean_b64.trim_end_matches('=');
        let decoded = data_encoding::BASE64URL_NOPAD
            .decode(unpadded.as_bytes())
            .or_else(|_| data_encoding::BASE64URL.decode(clean_b64.as_bytes()))
            .or_else(|_| data_encoding::BASE64.decode(clean_b64.as_bytes()))
            .or_else(|_| data_encoding::BASE64_NOPAD.decode(unpadded.as_bytes()));

        match decoded {
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

    let peer_ip = peer_addr.ip();
    let is_trusted_peer = peer_ip.is_loopback() || match peer_ip {
        std::net::IpAddr::V4(v4) => v4.is_private(),
        std::net::IpAddr::V6(v6) => (v6.segments()[0] & 0xfe00) == 0xfc00,
    };

    let client_ip = if is_trusted_peer {
        headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok())
            .unwrap_or(peer_ip)
    } else {
        peer_ip
    };

    let mut ctx = crate::pipeline::context::QueryContext::new(client_ip, msg);
    state.engine.process(&mut ctx).await;

    state.metrics.record(&ctx);
    if *state.logging_server_enabled.read() {
        state.db.record_query(&ctx);
    }

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

    // Web 控制台端口严格隔离: 对 DoH 路径直接返回 404，不允许 SPA fallback 接管
    if path == "dns-query" || path.starts_with("dns-query/") {
        return (StatusCode::NOT_FOUND, "404 Not Found").into_response();
    }

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
        version: env!("CARGO_PKG_VERSION"),
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

async fn get_cache_stats(State(state): State<AppState>) -> Json<crate::metrics::CacheStatsSnapshot> {
    Json(state.metrics.cache_stats.snapshot())
}

async fn clear_cache_stats(State(state): State<AppState>) -> StatusCode {
    state.metrics.cache_stats.clear();
    let snap = state.metrics.cache_stats.snapshot();
    if let Ok(json) = serde_json::to_string(&snap) {
        let _ = state.db.set_system_setting("dns_cache_stats", &json);
    }
    StatusCode::OK
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
            description: "内存零拷贝极速秒级应答与高频热点记录静默预取",
            enabled: state.engine.cache_enabled.load(Ordering::Relaxed),
        },
        PipelineNode {
            id: "router",
            name: "智能分流调度引擎",
            stage: "routing",
            description: "集成 Direct/Proxy 分流、未命中智能回退与非大陆 IP 防污染探测，带优雅降级保底",
            enabled: state.engine.router_enabled.load(Ordering::Relaxed),
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
) -> Result<StatusCode, (StatusCode, String)> {
    if payload.id == "router" && payload.enabled {
        let cfg = state.config.read();
        let gfw_has_server = cfg.upstreams.gfw.iter().any(|s| !s.trim().is_empty());
        let overseas_has_server = cfg.upstreams.overseas.iter().any(|s| !s.trim().is_empty());
        if !gfw_has_server || !overseas_has_server {
            return Err((
                StatusCode::BAD_REQUEST,
                "无法开启动态分流调度引擎：GFW / 境外组 (gfw) 与海外备用组 (overseas) 必须各自至少配置一个上游 DNS 服务器".to_string(),
            ));
        }
    }

    if state.engine.toggle_stage(&payload.id, payload.enabled) {
        // 同步持久化写入 config.yaml，确保刷新或切换页面绝不丢失
        {
            let mut cfg = state.config.write();
            match payload.id.as_str() {
                "router" => cfg.routing.enabled = payload.enabled,
                "speed_check" => cfg.speed_check.enabled = payload.enabled,
                _ => {}
            }
            let _ = cfg.save(&state.config_path);
        }
        Ok(StatusCode::OK)
    } else {
        Err((StatusCode::BAD_REQUEST, "无法切换该节点状态".to_string()))
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
        updated_at: Local::now().to_rfc3339(),
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
        r.updated_at = Local::now().to_rfc3339();

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
                if let Ok(Some(c)) = conn.query_row(
                    "SELECT content FROM adblock_rules WHERE id = ?1",
                    rusqlite::params![r.id],
                    |row| row.get::<_, Option<String>>(0),
                ) {
                    combined.push_str(&c);
                    combined.push('\n');
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
    bootstrap_dns: Vec<String>,
    mainland: Vec<String>,
    gfw: Vec<String>,
    overseas: Vec<String>,
}

async fn get_upstreams(State(state): State<AppState>) -> Json<UpstreamsResponse> {
    let cfg = state.config.read();
    Json(UpstreamsResponse {
        bootstrap_dns: cfg.upstreams.bootstrap_dns.clone(),
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

async fn probe_upstream_latency(
    group: String,
    addr: String,
    proxy_cfg: crate::config::ProxyConfig,
) -> UpstreamTestItem {
    let start = std::time::Instant::now();
    let server = crate::pipeline::plugins::upstream::UpstreamServer::parse(&group, &addr);
    let mut ok = false;
    let mut rtt = None;

    if let Some(mut s) = server {
        let proxy_mode = s.proxy.clone().unwrap_or_else(|| {
            if group == "mainland" || group == "bootstrap" { "none".to_string() } else { "default".to_string() }
        });
        s.set_proxy(proxy_cfg.find_proxy_addr(&proxy_mode));
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

    UpstreamTestItem {
        addr,
        group,
        latency_ms: rtt,
        ok,
    }
}

async fn test_upstreams(State(state): State<AppState>) -> Json<Vec<UpstreamTestItem>> {
    let (bootstrap, mainland, gfw, overseas, proxy_cfg) = {
        let cfg = state.config.read();
        (
            cfg.upstreams.bootstrap_dns.clone(),
            cfg.upstreams.mainland.clone(),
            cfg.upstreams.gfw.clone(),
            cfg.upstreams.overseas.clone(),
            cfg.proxy.clone(),
        )
    };

    let groups = [
        ("bootstrap", bootstrap),
        ("mainland", mainland),
        ("gfw", gfw),
        ("overseas", overseas),
    ];

    let mut tasks = Vec::new();
    for (group_name, list) in groups {
        for addr in list {
            let g = group_name.to_string();
            let p = proxy_cfg.clone();
            tasks.push(tokio::spawn(async move {
                probe_upstream_latency(g, addr, p).await
            }));
        }
    }

    let mut results = Vec::new();
    for t in tasks {
        if let Ok(item) = t.await {
            results.push(item);
        }
    }

    Json(results)
}

#[derive(Deserialize)]
struct GroupUpstreamTestPayload {
    group: String,
}

async fn test_group_upstreams(
    State(state): State<AppState>,
    Json(payload): Json<GroupUpstreamTestPayload>,
) -> Result<Json<Vec<UpstreamTestItem>>, (StatusCode, String)> {
    let (servers, proxy_cfg) = {
        let cfg = state.config.read();
        let s = match payload.group.as_str() {
            "bootstrap" => cfg.upstreams.bootstrap_dns.clone(),
            "mainland" => cfg.upstreams.mainland.clone(),
            "gfw" => cfg.upstreams.gfw.clone(),
            "overseas" => cfg.upstreams.overseas.clone(),
            _ => return Err((StatusCode::BAD_REQUEST, "未知上游分组".to_string())),
        };
        (s, cfg.proxy.clone())
    };

    let group_name = payload.group;
    let mut tasks = Vec::new();
    for addr in servers {
        let g = group_name.clone();
        let p = proxy_cfg.clone();
        tasks.push(tokio::spawn(async move {
            probe_upstream_latency(g, addr, p).await
        }));
    }

    let mut results = Vec::new();
    for t in tasks {
        if let Ok(item) = t.await {
            results.push(item);
        }
    }

    Ok(Json(results))
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

async fn sse_querylog_handler(
    State(state): State<AppState>,
) -> axum::response::sse::Sse<impl futures_util::Stream<Item = Result<axum::response::sse::Event, std::convert::Infallible>>> {
    let rx = state.db.subscribe_ws();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(item) => {
                    if let Ok(json) = serde_json::to_string(&item) {
                        return Some((Ok(axum::response::sse::Event::default().data(json)), rx));
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
            }
        }
    });

    axum::response::sse::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
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
            rusqlite::params![new_id, Local::now().to_rfc3339()],
        );
        new_id
    });

    conn.execute(
        "UPDATE adblock_rules SET content = coalesce(content, '') || ?1, updated_at = ?2, rule_count = rule_count + 1 WHERE id = ?3",
        rusqlite::params![line, Local::now().to_rfc3339(), rule_id],
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
    if payload.group == "bootstrap" {
        if let Err(e) = validate_bootstrap_addr(&payload.addr) {
            return Err((StatusCode::BAD_REQUEST, e));
        }
    }
    {
        let mut cfg = state.config.write();
        match payload.group.as_str() {
            "bootstrap" => cfg.upstreams.bootstrap_dns.push(payload.addr),
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
            if payload.group == "mainland" || payload.group == "bootstrap" { "none".to_string() } else { "default".to_string() }
        });
        s.set_proxy(proxy_cfg.find_proxy_addr(&proxy_mode));

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
    if payload.new_group == "bootstrap" {
        if let Err(e) = validate_bootstrap_addr(&payload.new_addr) {
            return Err((StatusCode::BAD_REQUEST, e));
        }
    }
    {
        let mut cfg = state.config.write();
        let old_target = payload.old_addr.trim();
        let same_group = payload.old_group == payload.new_group;

        let replaced = match payload.old_group.as_str() {
            "bootstrap" => {
                if same_group {
                    if let Some(pos) = cfg.upstreams.bootstrap_dns.iter().position(|s| s.trim() == old_target) {
                        cfg.upstreams.bootstrap_dns[pos] = payload.new_addr.clone();
                        true
                    } else {
                        cfg.upstreams.bootstrap_dns.retain(|s| s.trim() != old_target);
                        false
                    }
                } else {
                    cfg.upstreams.bootstrap_dns.retain(|s| s.trim() != old_target);
                    false
                }
            }
            "mainland" => {
                if same_group {
                    if let Some(pos) = cfg.upstreams.mainland.iter().position(|s| s.trim() == old_target) {
                        cfg.upstreams.mainland[pos] = payload.new_addr.clone();
                        true
                    } else {
                        cfg.upstreams.mainland.retain(|s| s.trim() != old_target);
                        false
                    }
                } else {
                    cfg.upstreams.mainland.retain(|s| s.trim() != old_target);
                    false
                }
            }
            "gfw" => {
                if same_group {
                    if let Some(pos) = cfg.upstreams.gfw.iter().position(|s| s.trim() == old_target) {
                        cfg.upstreams.gfw[pos] = payload.new_addr.clone();
                        true
                    } else {
                        cfg.upstreams.gfw.retain(|s| s.trim() != old_target);
                        false
                    }
                } else {
                    cfg.upstreams.gfw.retain(|s| s.trim() != old_target);
                    false
                }
            }
            "overseas" => {
                if same_group {
                    if let Some(pos) = cfg.upstreams.overseas.iter().position(|s| s.trim() == old_target) {
                        cfg.upstreams.overseas[pos] = payload.new_addr.clone();
                        true
                    } else {
                        cfg.upstreams.overseas.retain(|s| s.trim() != old_target);
                        false
                    }
                } else {
                    cfg.upstreams.overseas.retain(|s| s.trim() != old_target);
                    false
                }
            }
            _ => return Err((StatusCode::BAD_REQUEST, "未知旧分组".to_string())),
        };

        if !replaced {
            match payload.new_group.as_str() {
                "bootstrap" => cfg.upstreams.bootstrap_dns.push(payload.new_addr),
                "mainland" => cfg.upstreams.mainland.push(payload.new_addr),
                "gfw" => cfg.upstreams.gfw.push(payload.new_addr),
                "overseas" => cfg.upstreams.overseas.push(payload.new_addr),
                _ => return Err((StatusCode::BAD_REQUEST, "未知新分组".to_string())),
            }
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
            "bootstrap" => cfg.upstreams.bootstrap_dns = payload.addrs,
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
            "bootstrap" => {
                if cfg.upstreams.bootstrap_dns.len() <= 1 {
                    return Err((StatusCode::BAD_REQUEST, "引导 DNS 至少需保留一个节点".to_string()));
                }
                cfg.upstreams.bootstrap_dns.retain(|s| s != &payload.addr);
            }
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

#[derive(Deserialize)]
struct UpdateBootstrapPayload {
    dns_list: Vec<String>,
}

async fn update_bootstrap_dns(
    State(state): State<AppState>,
    Json(payload): Json<UpdateBootstrapPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    for b in &payload.dns_list {
        if let Err(e) = validate_bootstrap_addr(b) {
            return Err((StatusCode::BAD_REQUEST, e));
        }
    }
    {
        let mut cfg = state.config.write();
        cfg.upstreams.bootstrap_dns = payload.dns_list;
        let _ = cfg.save(&state.config_path);
    }
    Ok(StatusCode::OK)
}

fn validate_bootstrap_addr(addr_str: &str) -> Result<(), String> {
    let raw = addr_str.trim();
    if raw.is_empty() {
        return Err("引导 DNS 地址不能为空".to_string());
    }
    let base = raw.split('#').next().unwrap_or(raw);
    let without_scheme = if let Some(idx) = base.find("://") {
        &base[idx + 3..]
    } else {
        base
    };
    let host_port = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host = if host_port.starts_with('[') {
        let end = host_port.find(']').unwrap_or(host_port.len());
        &host_port[1..end]
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    if host.parse::<std::net::IpAddr>().is_err() {
        return Err(format!("引导 DNS 主机必须是纯 IP 地址，禁止使用域名: {} (提取的主机名: {})", addr_str, host));
    }
    Ok(())
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
        rusqlite::params![payload.content, count as i64, Local::now().to_rfc3339(), id],
    ).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    reload_all_rules(&state).await;
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct RoutingRuleContentResponse {
    file_name: String,
    content: String,
}

fn validate_rule_filename(name: &str) -> Result<(), (StatusCode, String)> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.starts_with('.')
    {
        return Err((StatusCode::BAD_REQUEST, "非法文件名格式，严禁包含路径穿越字符".to_string()));
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.') {
        return Err((StatusCode::BAD_REQUEST, "文件名包含非法字符，仅允许字母、数字、点、横杠与下划线".to_string()));
    }
    Ok(())
}

async fn get_routing_rule_content(
    State(state): State<AppState>,
    AxumPath(file_name): AxumPath<String>,
) -> Result<Json<RoutingRuleContentResponse>, (StatusCode, String)> {
    validate_rule_filename(&file_name)?;
    let path = state.rule_manager.rules_dir().join(&file_name);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    Ok(Json(RoutingRuleContentResponse { file_name, content }))
}

async fn update_routing_rule_content(
    State(state): State<AppState>,
    AxumPath(file_name): AxumPath<String>,
    Json(payload): Json<UpdateRuleContentPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_rule_filename(&file_name)?;
    let path = state.rule_manager.rules_dir().join(&file_name);
    std::fs::write(&path, &payload.content).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    state.rule_manager.load_local_rules();
    Ok(StatusCode::OK)
}

#[derive(Serialize)]
struct RawConfigResponse {
    content: String,
}

async fn get_raw_config(State(state): State<AppState>) -> Result<Json<RawConfigResponse>, (StatusCode, String)> {
    let content = std::fs::read_to_string(&state.config_path).unwrap_or_default();
    Ok(Json(RawConfigResponse { content }))
}

async fn update_raw_config(
    State(state): State<AppState>,
    Json(payload): Json<UpdateRuleContentPayload>,
) -> Result<StatusCode, (StatusCode, String)> {
    let parsed: crate::config::AppConfig = serde_yaml::from_str(&payload.content)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("Invalid YAML: {}", e)))?;
    validate_config_safety(&parsed)?;
    std::fs::write(&state.config_path, &payload.content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    apply_config_to_runtime(&state, &parsed);
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

#[derive(Deserialize)]
struct ToolQueryPayload {
    domain: String,
    #[serde(default = "default_tool_qtype")]
    qtype: String,
}

fn default_tool_qtype() -> String {
    "A".to_string()
}

#[derive(Serialize)]
struct ToolAnswerItem {
    record_type: String,
    data: String,
    ttl: u32,
}

#[derive(Serialize)]
struct ToolQueryResponse {
    domain: String,
    qtype: String,
    status: String,
    outcome: String,
    rcode: String,
    duration_ms: f64,
    answers_count: usize,
    routing_group: Option<String>,
    matched_rule: Option<String>,
    answers: Vec<ToolAnswerItem>,
    summary: String,
}

async fn tool_query_dns(
    State(state): State<AppState>,
    Json(payload): Json<ToolQueryPayload>,
) -> Result<Json<ToolQueryResponse>, (StatusCode, String)> {
    use hickory_proto::rr::RecordType;
    use std::str::FromStr;

    let domain_clean = payload.domain.trim().trim_end_matches('.').to_lowercase();
    if domain_clean.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "域名不能为空".to_string()));
    }

    let qtype = match payload.qtype.trim().to_uppercase().as_str() {
        "A" => RecordType::A,
        "AAAA" => RecordType::AAAA,
        "CNAME" => RecordType::CNAME,
        "MX" => RecordType::MX,
        "TXT" => RecordType::TXT,
        "NS" => RecordType::NS,
        "PTR" => RecordType::PTR,
        "SOA" => RecordType::SOA,
        "SRV" => RecordType::SRV,
        _ => RecordType::A,
    };

    let mut req_msg = hickory_proto::op::Message::query();
    let fqdn = format!("{}.", domain_clean);
    let name = hickory_proto::rr::Name::from_str(&fqdn)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("非法域名格式: {}", e)))?;

    req_msg.add_query(hickory_proto::op::Query::query(name, qtype));

    let client_ip = std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));
    let mut ctx = crate::pipeline::context::QueryContext::new(client_ip, req_msg);

    state.engine.process(&mut ctx).await;
    tracing::info!("Tool Query tags: {:?}, timings: {:?}", ctx.tags, ctx.timings);

    let mut answers = Vec::new();
    let mut answers_count = 0;

    if let Some(ref resp) = ctx.response {
        answers_count = resp.answers.len();
        for ans in &resp.answers {
            let record_str = match &ans.data {
                hickory_proto::rr::RData::A(a) => a.0.to_string(),
                hickory_proto::rr::RData::AAAA(aaaa) => aaaa.0.to_string(),
                hickory_proto::rr::RData::CNAME(cname) => cname.to_utf8().trim_end_matches('.').to_string(),
                hickory_proto::rr::RData::MX(mx) => format!("{} (pref: {})", mx.exchange.to_utf8().trim_end_matches('.'), mx.preference),
                hickory_proto::rr::RData::TXT(txt) => txt.txt_data.iter().map(|b| String::from_utf8_lossy(b).to_string()).collect::<Vec<_>>().join(" "),
                hickory_proto::rr::RData::PTR(ptr) => ptr.to_utf8().trim_end_matches('.').to_string(),
                hickory_proto::rr::RData::NS(ns) => ns.to_utf8().trim_end_matches('.').to_string(),
                _ => format!("{:?}", ans.data),
            };
            answers.push(ToolAnswerItem {
                record_type: ans.record_type().to_string(),
                data: record_str,
                ttl: ans.ttl,
            });
        }
    }

    let status_str = match ctx.status {
        crate::pipeline::context::QueryStatus::Allowed => "allowed",
        crate::pipeline::context::QueryStatus::Blocked => "blocked",
        crate::pipeline::context::QueryStatus::Cached => "cached",
        crate::pipeline::context::QueryStatus::Rewritten => "rewritten",
        crate::pipeline::context::QueryStatus::Error => "error",
    };

    let outcome = match ctx.status {
        crate::pipeline::context::QueryStatus::Blocked => "BLOCKED".to_string(),
        crate::pipeline::context::QueryStatus::Cached => "CACHED".to_string(),
        crate::pipeline::context::QueryStatus::Rewritten => "REWRITTEN".to_string(),
        crate::pipeline::context::QueryStatus::Allowed => {
            if ctx.rcode == hickory_proto::op::ResponseCode::NoError {
                "RESOLVED".to_string()
            } else {
                "ERROR".to_string()
            }
        }
        crate::pipeline::context::QueryStatus::Error => "ERROR".to_string(),
    };

    let summary = if outcome == "BLOCKED" {
        format!("BLOCKED ({})", ctx.matched_rule.as_deref().unwrap_or("rule_matched"))
    } else if outcome == "CACHED" {
        "RESOLVED (cache: hit_memory)".to_string()
    } else if outcome == "REWRITTEN" {
        "RESOLVED (local: rewrite_hosts)".to_string()
    } else if let Some(ref g) = ctx.routing_group {
        format!("RESOLVED (upstream: {})", g)
    } else {
        format!("{} ({})", outcome, ctx.rcode)
    };

    Ok(Json(ToolQueryResponse {
        domain: domain_clean,
        qtype: qtype.to_string(),
        status: status_str.to_string(),
        outcome,
        rcode: ctx.rcode.to_string(),
        duration_ms: (ctx.duration_ms * 100.0).round() / 100.0,
        answers_count,
        routing_group: ctx.routing_group,
        matched_rule: ctx.matched_rule,
        answers,
        summary,
    }))
}
