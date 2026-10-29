use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use clap::Parser;
use tower_http::cors::CorsLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod config;
mod db;
mod metrics;
mod pipeline;
mod server;

use api::{create_router, AppState};
use config::AppConfig;
use db::Database;
use metrics::MetricsCollector;
use pipeline::engine::PipelineEngine;
use pipeline::plugins::adblock::{AdblockPlugin, BlockType};
use pipeline::plugins::cache::CachePlugin;
use pipeline::plugins::domain_match::{DomainListRegistry, DomainMatcher};
use pipeline::plugins::ip_match::IpListRegistry;
use pipeline::plugins::qtype_drop::QtypeDropPlugin;
use pipeline::plugins::rewrite::{RewritePlugin, RewriteTarget};
use pipeline::plugins::speed_check::SpeedCheckPlugin;
use pipeline::plugins::upstream::{ForwardPlugin, UpstreamGroup, UpstreamServer};
use pipeline::rule_files::RuleFileManager;
use server::DnsServer;

#[derive(Parser, Debug)]
#[command(name = "nexdns", version = "0.1.0", about = "高性能融合型 DNS 原生服务")]
struct Cli {
    #[arg(short, long, default_value = "data/config.yaml")]
    config: PathBuf,

    #[arg(long)]
    dns_port: Option<u16>,

    #[arg(long)]
    web_port: Option<u16>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "nexdns=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();
    tracing::info!("启动 NexDNS 融合单内核服务...");

    let mut cfg = AppConfig::load_or_create(&cli.config)?;
    if let Some(p) = cli.dns_port {
        cfg.dns.port = p;
    }
    if let Some(p) = cli.web_port {
        cfg.web.port = p;
    }
    let config = Arc::new(cfg);

    let db_path = cli.config
        .parent()
        .map(|p| p.join("nexdns.db"))
        .unwrap_or_else(|| std::path::PathBuf::from("data/nexdns.db"));
    let db_path_str = db_path.to_string_lossy().to_string();

    // Init DB
    let db = Database::init(&db_path)?;
    tracing::info!("SQLite WAL 数据库初始化成功 ({})", db_path_str);

    // Init Metrics
    let metrics = MetricsCollector::new();

    // 1. Plugins: QtypeDrop
    let qtype_drop = Arc::new(QtypeDropPlugin::new(65, hickory_proto::op::ResponseCode::NXDomain));

    // 2. Plugins: Rewrite
    let rewrite = Arc::new(RewritePlugin::new());
    if let Ok(rewrites) = db.get_rewrites() {
        let mut map = std::collections::HashMap::new();
        for r in rewrites {
            if r.enabled {
                if let Ok(ip) = r.value.parse::<std::net::IpAddr>() {
                    map.entry(r.domain).or_insert_with(Vec::new).push(RewriteTarget::Ip(ip));
                } else {
                    map.entry(r.domain).or_insert_with(Vec::new).push(RewriteTarget::Cname(r.value));
                }
            }
        }
        if !map.is_empty() {
            rewrite.set_records(map);
        }
    }

    // 3. Plugins: Adblock
    let adblock = Arc::new(AdblockPlugin::new(BlockType::ZeroIp));
    // Seed default common ad rules if DB empty
    if let Ok(existing_rules) = db.get_rules() {
        if existing_rules.is_empty() {
            let default_ad_rules = "||adservice.google.com^\n||ads.youtube.com^\n||ad.doubleclick.net^\n||pagead2.googlesyndication.com^\n||analytics.google.com^\n||pos.baidu.com^\n||cpro.baidu.com^\n||eclick.baidu.com^\n||ad.xiaomi.com^\n||tracking.miui.com^\n0.0.0.0 telemetry.microsoft.com";
            let seed_rec = db::AdblockRuleRecord {
                id: uuid::Uuid::new_v4().to_string(),
                name: "基础推荐拦截列表".to_string(),
                url: "built-in".to_string(),
                enabled: true,
                rule_count: 11,
                updated_at: chrono::Utc::now().to_rfc3339(),
            };
            let _ = db.save_rule(&seed_rec, default_ad_rules);
            adblock.load_rules_from_text(default_ad_rules);
        } else {
            let mut combined = String::new();
            if let Ok(conn) = db.get_conn() {
                for r in existing_rules {
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
            }
            adblock.load_rules_from_text(&combined);
        }
    }

    // 4. Plugins: Cache
    let cache = Arc::new(CachePlugin::new(
        config.cache.size,
        config.cache.serve_expired,
        config.cache.prefetch,
        config.cache.min_ttl,
        config.cache.max_ttl,
    ));

    // 5. Plugins: Domain Registry (Routing lists)
    let domain_registry = Arc::new(DomainListRegistry::new());
    // 6. Plugins: IP Registry
    let ip_registry = Arc::new(IpListRegistry::new());

    let rules_dir = cli.config
        .parent()
        .map(|p| p.join("rules"))
        .unwrap_or_else(|| std::path::PathBuf::from("data/rules"));

    // Rule File Manager (loads direct.txt, proxy.txt, private.txt, ipv4_china.txt from data/rules)
    let rule_manager = Arc::new(RuleFileManager::new(
        rules_dir,
        &db_path_str,
        domain_registry.clone(),
        ip_registry.clone(),
    ));
    rule_manager.load_local_rules();

    // Seed built-in minimal fallbacks if rule files do not exist yet
    if !domain_registry.matches("domain_direct", "baidu.com") {
        let mut direct_matcher = DomainMatcher::new("domain_direct");
        direct_matcher.load_from_text(
            ".cn\n+.baidu.com\n+.qq.com\n+.aliyun.com\n+.taobao.com\n+.jd.com\n+.bilibili.com\n+.zhihu.com\n+.163.com\n+.weibo.com\n+.tencent.com\n+.douyin.com\n+.bytedance.com"
        );
        domain_registry.register(direct_matcher);
    }
    if !domain_registry.matches("domain_proxy", "google.com") {
        let mut proxy_matcher = DomainMatcher::new("domain_proxy");
        proxy_matcher.load_from_text(
            "+.google.com\n+.youtube.com\n+.github.com\n+.twitter.com\n+.x.com\n+.openai.com\n+.cloudflare.com\n+.facebook.com\n+.instagram.com\n+.wikipedia.org"
        );
        domain_registry.register(proxy_matcher);
    }
    if !domain_registry.matches("domain_private", "nas.home") {
        let mut private_matcher = DomainMatcher::new("domain_private");
        private_matcher.load_from_text("+.local\n+.lan\n+.internal\n+.home.arpa\n+.corp");
        domain_registry.register(private_matcher);
    }
    // 7. Plugins: Upstreams
    let mainland_servers: Vec<UpstreamServer> = config
        .upstreams
        .mainland
        .iter()
        .filter_map(|u| {
            let mut s = UpstreamServer::parse("mainland", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "none".to_string());
            s.proxy = config.proxy.find_proxy_addr(&proxy_mode);
            Some(s)
        })
        .collect();
    let gfw_servers: Vec<UpstreamServer> = config
        .upstreams
        .gfw
        .iter()
        .filter_map(|u| {
            let mut s = UpstreamServer::parse("gfw", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "default".to_string());
            s.proxy = config.proxy.find_proxy_addr(&proxy_mode);
            Some(s)
        })
        .collect();
    let overseas_servers: Vec<UpstreamServer> = config
        .upstreams
        .overseas
        .iter()
        .filter_map(|u| {
            let mut s = UpstreamServer::parse("overseas", u)?;
            let proxy_mode = s.proxy.clone().unwrap_or_else(|| "default".to_string());
            s.proxy = config.proxy.find_proxy_addr(&proxy_mode);
            Some(s)
        })
        .collect();

    let forward_mainland = Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new(
        "mainland",
        mainland_servers,
        config.upstreams.timeout_ms,
    ))));
    let forward_gfw = Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new(
        "gfw",
        gfw_servers,
        config.upstreams.timeout_ms,
    ))));
    let forward_overseas = Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new(
        "overseas",
        overseas_servers,
        config.upstreams.timeout_ms,
    ))));

    // 8. Plugins: Speed Check (TCP 80/443 handshake)
    let speed_check = Arc::new(SpeedCheckPlugin::new(
        config.speed_check.enabled,
        config.speed_check.timeout_ms,
        config.speed_check.cache_mins,
    ));

    // 9. Pipeline Engine
    let engine = Arc::new(PipelineEngine::new(
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
        config.anti_pollution,
        config.dns.response_mode.clone(),
    ));

    // 10. Start DNS Server (Controlled by independent enabled switch)
    let dns_server_enabled = Arc::new(parking_lot::RwLock::new(config.dns.enabled));

    let dns_server = DnsServer::new(
        config.dns.port,
        engine.clone(),
        db.clone(),
        metrics.clone(),
        dns_server_enabled.clone(),
    );
    dns_server.start().await?;

    let shared_config = Arc::new(parking_lot::RwLock::new((*config).clone()));

    // 11. Start Axum Web Server
    let app_state = AppState {
        engine,
        db,
        metrics,
        config: shared_config.clone(),
        config_path: cli.config.to_string_lossy().to_string(),
        dns_server_enabled,
        rule_manager,
        start_time: std::time::Instant::now(),
    };

    // 12. 后台异步规则文件自动更新调度器 (不阻塞程序启动，10秒后首次检测)
    let auto_update_state = app_state.clone();

    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        loop {
            let (enabled, interval_mins) = {
                let c = auto_update_state.config.read();
                (c.rules_update.auto_update, c.rules_update.interval_mins.max(1))
            };

            if enabled {
                tracing::info!("后台调度: 开始自动更新在线规则文件 (间隔: {} min)...", interval_mins);
                // 1. 更新拦截规则库中配置了在线 URL 的项
                if let Ok(rules) = auto_update_state.db.get_rules() {
                    for r in rules {
                        if r.enabled && r.url.starts_with("http") {
                            let client = reqwest::Client::builder()
                                .timeout(std::time::Duration::from_secs(15))
                                .build();
                            if let Ok(client) = client {
                                if let Ok(resp) = client.get(&r.url).send().await {
                                    if let Ok(text) = resp.text().await {
                                        let mut store = crate::pipeline::plugins::adblock::RuleStore::default();
                                        store.parse_rules(&text);
                                        let mut updated = r.clone();
                                        updated.rule_count = store.total_count;
                                        updated.updated_at = chrono::Utc::now().to_rfc3339();
                                        let _ = auto_update_state.db.save_rule(&updated, &text);
                                    }
                                }
                            }
                        }
                    }
                    crate::api::reload_all_rules(&auto_update_state).await;
                }

                // 2. 更新分流规则集中配置了在线 URL 的项
                auto_update_state.rule_manager.update_all().await;
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(interval_mins * 60)).await;
        }
    });

    let app = create_router(app_state)
        .layer(CorsLayer::permissive());

    // Optional independent DoH HTTP listener
    if config.dns.doh_enabled && config.dns.doh_port > 0 && config.dns.doh_port != config.web.port {
        let doh_port = config.dns.doh_port;
        let doh_router = app.clone();
        tokio::spawn(async move {
            let doh_addr: SocketAddr = format!("0.0.0.0:{}", doh_port).parse().unwrap();
            if let Ok(listener) = tokio::net::TcpListener::bind(doh_addr).await {
                tracing::info!("DoH 独立服务监听在 http://{}/dns-query", doh_addr);
                let _ = axum::serve(listener, doh_router.into_make_service_with_connect_info::<SocketAddr>()).await;
            } else {
                tracing::warn!("无法绑定 DoH 独立端口: {}", doh_port);
            }
        });
    }

    let web_addr: SocketAddr = format!("0.0.0.0:{}", config.web.port).parse()?;
    tracing::info!("Web 控制台 API 服务运行在 http://{}", web_addr);

    let listener = tokio::net::TcpListener::bind(web_addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>()).await?;

    Ok(())
}
