use std::net::Ipv4Addr;
use std::str::FromStr;
use hickory_proto::op::Message;
use hickory_proto::rr::rdata::A;
use hickory_proto::rr::{Name, RData, Record, RecordType};

#[test]
fn test_rule_store_abp_and_whitelist() {
    // Test Adblock rule store
    let mut store = nexdns::pipeline::plugins::adblock::RuleStore::default();
    store.parse_rules("
        ||ads.google.com^
        ||doubleclick.net^
        @@||allowed.doubleclick.net^
        0.0.0.0 tracking.example.com
        /^bad[0-9]+\\.com/
    ");

    assert!(store.match_deny("ads.google.com").is_some());
    assert!(store.match_deny("sub.ads.google.com").is_some());
    assert!(store.match_deny("doubleclick.net").is_some());
    // Whitelist check
    assert!(store.match_deny("allowed.doubleclick.net").is_none());
    // Hosts check
    assert!(store.match_deny("tracking.example.com").is_some());
    // Regex check
    assert!(store.match_deny("bad123.com").is_some());
    assert!(store.match_deny("goodsite.com").is_none());
}

#[test]
fn test_domain_matcher() {
    let mut matcher = nexdns::pipeline::plugins::domain_match::DomainMatcher::new("test");
    matcher.load_from_text(".cn\n+.baidu.com\nqq.com");

    assert!(matcher.matches("test.cn"));
    assert!(matcher.matches("www.baidu.com"));
    assert!(matcher.matches("pan.baidu.com"));
    assert!(matcher.matches("qq.com"));
    assert!(!matcher.matches("google.com"));
}

#[test]
fn test_domain_matcher_prefixes() {
    let mut matcher = nexdns::pipeline::plugins::domain_match::DomainMatcher::new("test");
    matcher.load_from_text("domain:google.com\nfull:youtube.com\nkeyword:facebook");

    assert!(matcher.matches("google.com"));
    assert!(matcher.matches("www.google.com"));
    assert!(matcher.matches("youtube.com"));
    assert!(!matcher.matches("sub.youtube.com"));
    assert!(matcher.matches("facebook.com"));
}

#[test]
fn test_ip_matcher() {
    let mut matcher = nexdns::pipeline::plugins::ip_match::IpMatcher::new("test");
    matcher.load_from_text("192.168.0.0/16\n223.5.5.0/24");

    assert!(matcher.contains(&"192.168.1.1".parse().unwrap()));
    assert!(matcher.contains(&"223.5.5.5".parse().unwrap()));
    assert!(!matcher.contains(&"8.8.8.8".parse().unwrap()));
}

#[tokio::test]
async fn test_cache_standard_ttl() {
    let cache = nexdns::pipeline::plugins::cache::CachePlugin::new(100, false, 10, 3600);
    let name = Name::from_str("example.com.").unwrap();
    let mut msg = Message::query();
    let record = Record::from_rdata(name, 10, RData::A(A(Ipv4Addr::new(93, 184, 216, 34))));
    msg.answers.push(record);

    cache.insert("example.com", RecordType::A, msg);
    assert_eq!(cache.size(), 1);

    let res = cache.get("example.com", RecordType::A);
    assert!(res.is_some());
    let cached_msg = res.unwrap();
    assert_eq!(cached_msg.answers.len(), 1);
}

#[test]
fn test_upstream_server_parsing() {
    use nexdns::pipeline::plugins::upstream::{UpstreamServer, UpstreamProtocol};

    // Standard UDP
    let s1 = UpstreamServer::parse("mainland", "udp://223.5.5.5:53").unwrap();
    assert_eq!(s1.protocol, UpstreamProtocol::Udp);
    assert_eq!(s1.addr, "223.5.5.5:53");
    assert_eq!(s1.host_name, None);
    assert_eq!(s1.proxy, None);

    // DoH with NexDNS native URI fragment #sni=doh.360.cn
    let s2 = UpstreamServer::parse("mainland", "https://101.226.4.6/dns-query#sni=doh.360.cn").unwrap();
    assert_eq!(s2.protocol, UpstreamProtocol::Doh);
    assert_eq!(s2.addr, "https://101.226.4.6/dns-query");
    assert_eq!(s2.host_name.as_deref(), Some("doh.360.cn"));
    assert_eq!(s2.proxy, None);

    // DoH with NexDNS native URI fragment #sni=...&proxy=...
    let s3 = UpstreamServer::parse("gfw", "https://8.8.8.8/dns-query#sni=dns.google&proxy=default").unwrap();
    assert_eq!(s3.protocol, UpstreamProtocol::Doh);
    assert_eq!(s3.host_name.as_deref(), Some("dns.google"));
    assert_eq!(s3.proxy.as_deref(), Some("default"));

    // TCP with NexDNS native URI fragment #proxy=...
    let s4 = UpstreamServer::parse("overseas", "tcp://1.1.1.1:53#proxy=local_clash").unwrap();
    assert_eq!(s4.protocol, UpstreamProtocol::Tcp);
    assert_eq!(s4.proxy.as_deref(), Some("local_clash"));

    // Fallback compatibility with -host-name
    let s5 = UpstreamServer::parse("mainland", "https://101.226.4.6/dns-query -host-name doh.360.cn").unwrap();
    assert_eq!(s5.host_name.as_deref(), Some("doh.360.cn"));
}

#[test]
fn test_proxy_config_lookup() {
    use nexdns::config::{ProxyConfig, ProxyItem};

    let mut cfg = ProxyConfig::default();
    assert!(!cfg.enabled);
    assert!(cfg.socks5.is_none());
    assert!(cfg.list.is_empty());

    cfg.enabled = true;
    cfg.list = vec![
        ProxyItem {
            id: "p1".to_string(),
            name: "Node 1".to_string(),
            addr: "socks5://10.96.1.108:7890".to_string(),
            is_default: true,
        },
        ProxyItem {
            id: "p2".to_string(),
            name: "Node 2".to_string(),
            addr: "socks5://10.96.1.108:1080".to_string(),
            is_default: false,
        },
    ];

    assert_eq!(cfg.find_proxy_addr("none"), None);
    assert_eq!(cfg.find_proxy_addr("default").as_deref(), Some("socks5://10.96.1.108:7890"));
    assert_eq!(cfg.find_proxy_addr("p2").as_deref(), Some("socks5://10.96.1.108:1080"));
    assert_eq!(cfg.find_proxy_addr("socks5://127.0.0.1:9050").as_deref(), Some("socks5://127.0.0.1:9050"));

    // Disabled proxy returns None
    cfg.enabled = false;
    assert_eq!(cfg.find_proxy_addr("default"), None);
    assert_eq!(cfg.find_proxy_addr("p1"), None);
}

#[test]
fn test_cloudflare_doh_url_bypass() {
    let mut final_url = "https://cloudflare-dns.com/dns-query".to_string();
    let has_proxy = true;
    let is_cf_domain = final_url.contains("cloudflare-dns.com") || final_url.contains("dns.cloudflare.com");
    if has_proxy && is_cf_domain {
        final_url = final_url
            .replace("cloudflare-dns.com", "1.1.1.1")
            .replace("dns.cloudflare.com", "1.1.1.1");
    }
    assert_eq!(final_url, "https://1.1.1.1/dns-query");
}

#[tokio::test]
async fn test_alidns_doh() {
    use nexdns::pipeline::plugins::upstream::UpstreamServer;
    let s = UpstreamServer::parse("mainland", "https://223.5.5.5/dns-query#sni=dns.alidns.com").unwrap();
    let mut req = Message::query();
    req.add_query(hickory_proto::op::Query::query(
        Name::from_ascii("example.com.").unwrap(),
        RecordType::A,
    ));
    let bytes = req.to_vec().unwrap();
    let res = s.query(&bytes, std::time::Duration::from_millis(4000)).await;
    println!("Alidns result: {:?}", res);
    assert!(res.is_ok());
}
#[test]
fn test_reqwest_socks_scheme() {
    let p1 = reqwest::Proxy::all("socks5h://127.0.0.1:7890");
    println!("socks5h result: {:?}", p1.is_ok());
    assert!(p1.is_ok());

    let p2 = reqwest::Proxy::all("socks5://127.0.0.1:7890");
    println!("socks5 result: {:?}", p2.is_ok());
    assert!(p2.is_ok());
}

#[tokio::test]
async fn test_cache_intelligent_eviction() {
    use std::time::Duration;
    // Cache size 3, min_ttl 1, max_ttl 10
    let cache = nexdns::pipeline::plugins::cache::CachePlugin::new(3, false, 1, 10);
    let name = Name::from_str("expired.com.").unwrap();
    let mut msg_expired = Message::query();
    msg_expired.answers.push(Record::from_rdata(name, 1, RData::A(A(Ipv4Addr::new(1, 1, 1, 1)))));

    // Insert entry with 1s TTL
    cache.insert("expired.com", RecordType::A, msg_expired);

    // Insert fresh entry with 10s TTL
    let name2 = Name::from_str("fresh1.com.").unwrap();
    let mut msg_fresh1 = Message::query();
    msg_fresh1.answers.push(Record::from_rdata(name2, 10, RData::A(A(Ipv4Addr::new(2, 2, 2, 2)))));
    cache.insert("fresh1.com", RecordType::A, msg_fresh1);

    let name3 = Name::from_str("fresh2.com.").unwrap();
    let mut msg_fresh2 = Message::query();
    msg_fresh2.answers.push(Record::from_rdata(name3, 10, RData::A(A(Ipv4Addr::new(3, 3, 3, 3)))));
    cache.insert("fresh2.com", RecordType::A, msg_fresh2);

    assert_eq!(cache.size(), 3);

    // Wait 1.1s for expired.com to expire
    tokio::time::sleep(Duration::from_millis(1100)).await;

    // Insert 4th entry to trigger eviction
    let name4 = Name::from_str("fresh3.com.").unwrap();
    let mut msg_fresh3 = Message::query();
    msg_fresh3.answers.push(Record::from_rdata(name4, 10, RData::A(A(Ipv4Addr::new(4, 4, 4, 4)))));
    cache.insert("fresh3.com", RecordType::A, msg_fresh3);

    // Expired entry must be evicted, while fresh entries survive
    assert!(cache.get("expired.com", RecordType::A).is_none());
    assert!(cache.get("fresh1.com", RecordType::A).is_some());
    assert!(cache.get("fresh2.com", RecordType::A).is_some());
    assert!(cache.get("fresh3.com", RecordType::A).is_some());
}

#[test]
fn test_ip_matcher_intervals() {
    let mut matcher = nexdns::pipeline::plugins::ip_match::IpMatcher::new("test");
    matcher.load_from_text("10.0.0.0/8\n172.16.0.0/12\n192.168.0.0/16");

    assert!(matcher.contains(&"10.1.2.3".parse().unwrap()));
    assert!(matcher.contains(&"172.20.10.5".parse().unwrap()));
    assert!(matcher.contains(&"192.168.100.1".parse().unwrap()));
    assert!(!matcher.contains(&"11.0.0.1".parse().unwrap()));
    assert!(!matcher.contains(&"172.32.0.1".parse().unwrap()));
    assert!(!matcher.contains(&"192.169.0.1".parse().unwrap()));
}

#[test]
fn test_edns_cookie_echo_and_strip() {
    use hickory_proto::op::Edns;
    use hickory_proto::rr::rdata::opt::{EdnsCode, EdnsOption};

    let mut edns = Edns::new();
    let cookie_bytes = vec![1, 2, 3, 4, 5, 6, 7, 8];
    edns.options_mut().insert(EdnsOption::Unknown(10, cookie_bytes.clone()));

    let opt = edns.options().get(EdnsCode::Cookie);
    assert!(opt.is_some());

    if let Some(EdnsOption::Unknown(10, data)) = opt {
        assert_eq!(data, &cookie_bytes);
    } else {
        panic!("Expected cookie option");
    }
}

#[tokio::test]
async fn test_cname_precedes_address_records_in_speed_check() {
    use nexdns::pipeline::plugins::speed_check::SpeedCheckPlugin;
    use nexdns::pipeline::plugin::DnsPlugin;
    use nexdns::pipeline::context::QueryContext;

    let plugin = SpeedCheckPlugin::new(true, 100, 5);
    let mut resp = Message::response(1, hickory_proto::op::OpCode::Query);

    // Intentionally add A records and CNAME in mixed order
    let qname = Name::from_str("www.baidu.com.").unwrap();
    let target = Name::from_str("www.a.shifen.com.").unwrap();

    resp.answers.push(Record::from_rdata(target.clone(), 300, RData::A(A(Ipv4Addr::new(220, 181, 111, 1)))));
    resp.answers.push(Record::from_rdata(qname.clone(), 300, RData::CNAME(hickory_proto::rr::rdata::CNAME(target.clone()))));
    resp.answers.push(Record::from_rdata(target.clone(), 300, RData::A(A(Ipv4Addr::new(220, 181, 111, 2)))));

    let mut ctx = QueryContext::new("127.0.0.1".parse().unwrap(), Message::query());
    ctx.response = Some(resp);

    plugin.execute(&mut ctx).await;

    let out_resp = ctx.response.unwrap();
    assert_eq!(out_resp.answers.len(), 3);
    // First record MUST be CNAME!
    assert!(matches!(out_resp.answers[0].data, RData::CNAME(_)));
    assert_eq!(&out_resp.answers[0].name, &qname);
    // Subsequent records are A
    assert!(matches!(out_resp.answers[1].data, RData::A(_)));
    assert!(matches!(out_resp.answers[2].data, RData::A(_)));
}

#[test]
fn test_argon2_and_legacy_fallback() {
    use nexdns::db::auth::{hash_password, verify_password};
    use sha2::Digest;

    // 1. Argon2 hashing & verification
    let pass = "admin1234";
    let hash = hash_password(pass).expect("Argon2 hash should succeed");
    assert!(hash.starts_with("$argon2"));

    let (valid, should_upgrade) = verify_password(pass, &hash, "");
    assert!(valid);
    assert!(!should_upgrade);

    let (invalid, _) = verify_password("wrongpass", &hash, "");
    assert!(!invalid);

    // 2. Legacy SHA-256 fallback verification
    let salt = "test-salt-123";
    let mut hasher = sha2::Sha256::new();
    hasher.update(b"oldpassword");
    hasher.update(b":nexdns_salt:");
    hasher.update(salt.as_bytes());
    let legacy_hash = format!("{:x}", hasher.finalize());

    let (legacy_valid, upgrade_needed) = verify_password("oldpassword", &legacy_hash, salt);
    assert!(legacy_valid);
    assert!(upgrade_needed); // Must flag for automatic upgrade to Argon2!
}

#[test]
fn test_login_rate_limiting() {
    use nexdns::db::auth::AuthManager;

    let test_ip = "192.0.2.100";
    AuthManager::reset_login_attempts(test_ip);
    assert!(AuthManager::check_rate_limit(test_ip).is_ok());

    for _ in 0..4 {
        AuthManager::record_login_failure(test_ip);
        assert!(AuthManager::check_rate_limit(test_ip).is_ok());
    }

    // 5th failure triggers rate limit
    AuthManager::record_login_failure(test_ip);
    assert!(AuthManager::check_rate_limit(test_ip).is_err());

    // Reset clears the limit
    AuthManager::reset_login_attempts(test_ip);
    assert!(AuthManager::check_rate_limit(test_ip).is_ok());
}

#[tokio::test]
async fn test_dynamic_block_type_and_ttl() {
    use nexdns::pipeline::plugins::adblock::{AdblockPlugin, BlockType};
    use nexdns::pipeline::plugin::DnsPlugin;
    use nexdns::pipeline::context::QueryContext;
    use hickory_proto::op::ResponseCode;

    let plugin = AdblockPlugin::new(BlockType::ZeroIp, 45);
    plugin.load_rules_from_text("||ad.test.com^");

    // 1. ZeroIp test with custom TTL 45
    let mut req1 = Message::query();
    req1.add_query(hickory_proto::op::Query::query(
        Name::from_str("ad.test.com.").unwrap(),
        RecordType::A,
    ));
    let mut ctx1 = QueryContext::new("127.0.0.1".parse().unwrap(), req1);
    plugin.execute(&mut ctx1).await;
    let resp1 = ctx1.response.expect("Should have blocked response");
    assert_eq!(resp1.metadata.response_code, ResponseCode::NoError);
    assert_eq!(resp1.answers[0].ttl, 45);

    // 2. Dynamically change to NXDomain
    plugin.set_block_type(BlockType::NxDomain);
    let mut req2 = Message::query();
    req2.add_query(hickory_proto::op::Query::query(
        Name::from_str("ad.test.com.").unwrap(),
        RecordType::A,
    ));
    let mut ctx2 = QueryContext::new("127.0.0.1".parse().unwrap(), req2);
    plugin.execute(&mut ctx2).await;
    let resp2 = ctx2.response.expect("Should have blocked response");
    assert_eq!(resp2.metadata.response_code, ResponseCode::NXDomain);

    // 3. Dynamically change to Refused
    plugin.set_block_type(BlockType::Refused);
    let mut req3 = Message::query();
    req3.add_query(hickory_proto::op::Query::query(
        Name::from_str("ad.test.com.").unwrap(),
        RecordType::A,
    ));
    let mut ctx3 = QueryContext::new("127.0.0.1".parse().unwrap(), req3);
    plugin.execute(&mut ctx3).await;
    let resp3 = ctx3.response.expect("Should have blocked response");
    assert_eq!(resp3.metadata.response_code, ResponseCode::Refused);
}

#[tokio::test]
async fn test_cache_prefetch_trigger_under_20_percent_ttl() {
    use std::time::Duration;
    use nexdns::pipeline::plugins::cache::CachePlugin;

    let cache = CachePlugin::new(100, true, 2, 10);
    let name = Name::from_str("prefetch.example.com.").unwrap();
    let mut msg = Message::query();
    msg.answers.push(Record::from_rdata(name, 2, RData::A(A(Ipv4Addr::new(1, 2, 3, 4)))));

    cache.insert("prefetch.example.com", RecordType::A, msg);

    // Query immediately (100% TTL remaining): should NOT prefetch
    let (msg1, should_prefetch1) = cache.get_with_prefetch("prefetch.example.com", RecordType::A).unwrap();
    assert_eq!(msg1.answers.len(), 1);
    assert!(!should_prefetch1);

    // Sleep 1.65s (elapsed > 80%, remaining < 20%): MUST trigger prefetch
    tokio::time::sleep(Duration::from_millis(1650)).await;
    let (msg2, should_prefetch2) = cache.get_with_prefetch("prefetch.example.com", RecordType::A).unwrap();
    assert_eq!(msg2.answers.len(), 1);
    assert!(should_prefetch2);

    // Query immediately again: prefetched already flagged, should NOT duplicate
    let (_, should_prefetch3) = cache.get_with_prefetch("prefetch.example.com", RecordType::A).unwrap();
    assert!(!should_prefetch3);
}

#[test]
fn test_change_password_execution() {
    use nexdns::config::AuthConfig;
    use nexdns::db::auth::AuthManager;

    let mut auth_cfg = AuthConfig::default();

    // Default admin user exists
    let auth_res = AuthManager::authenticate(&mut auth_cfg, "admin", "admin1234").unwrap();
    assert!(auth_res.is_some());

    // Change password from admin1234 to newpass123
    let res = AuthManager::change_password(&mut auth_cfg, Some("admin"), "admin1234", "newpass123");
    assert!(res.is_ok(), "change_password should succeed: {:?}", res);
    assert!(auth_cfg.password.starts_with("$argon2"));

    // Verify old password no longer works
    let old_auth = AuthManager::authenticate(&mut auth_cfg, "admin", "admin1234").unwrap();
    assert!(old_auth.is_none());

    // Verify new password works
    let new_auth = AuthManager::authenticate(&mut auth_cfg, "admin", "newpass123").unwrap();
    assert!(new_auth.is_some());
}

#[test]
fn test_retention_days_migrated_and_stripped_from_yaml() {
    use nexdns::config::AppConfig;

    let yaml_with_legacy = r#"
logging:
  enabled: true
  retention_days: 1
"#;
    let mut cfg: AppConfig = serde_yaml::from_str(yaml_with_legacy).unwrap();
    cfg.normalize();
    // 1 day -> 1440 mins, and retention_days cleared
    assert_eq!(cfg.logging.retention_mins, 1440);
    assert!(cfg.logging.retention_days.is_none());

    let serialized = serde_yaml::to_string(&cfg).unwrap();
    assert!(!serialized.contains("retention_days"));
    assert!(serialized.contains("retention_mins: 1440"));
    assert!(serialized.contains("auth:"));
}

#[tokio::test]
async fn test_db_only_has_query_logs_and_cleans_up_expired_and_oversized_logs() {
    use nexdns::db::Database;

    let tmp_dir = std::env::temp_dir().join(format!("nexdns_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let db_path = tmp_dir.join("nexdns.db");
    let db = Database::init(&db_path).unwrap();

    // Verify ONLY query_logs table exists in SQLite (excluding sqlite_ internal tables)
    {
        let conn = db.get_conn().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(tables, vec!["query_logs".to_string()]);

        // Insert an expired log (2 hours ago) and a fresh log (now)
        let expired_ts = (chrono::Local::now() - chrono::Duration::minutes(120)).to_rfc3339();
        let fresh_ts = chrono::Local::now().to_rfc3339();
        conn.execute(
            "INSERT INTO query_logs (id, timestamp, client_ip, qname, qtype, status, rcode, duration_ms)
             VALUES ('id-1', ?1, '127.0.0.1', 'old.example.com', 'A', 'allowed', 'NoError', 1.2)",
            rusqlite::params![expired_ts],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO query_logs (id, timestamp, client_ip, qname, qtype, status, rcode, duration_ms)
             VALUES ('id-2', ?1, '127.0.0.1', 'fresh.example.com', 'A', 'allowed', 'NoError', 1.0)",
            rusqlite::params![fresh_ts],
        )
        .unwrap();
    }

    let deleted = db.cleanup_logs_older_than(60).unwrap();
    assert_eq!(deleted, 1);
    let (items, total) = db.query_logs(10, 0, None, None, None).unwrap();
    assert_eq!(total, 1);
    assert_eq!(items[0].qname, "fresh.example.com");

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_cache_hit_rate_only_counts_cache_path_queries() {
    use hickory_proto::op::Message;
    use nexdns::metrics::MetricsCollector;
    use nexdns::pipeline::context::{QueryContext, QueryStatus};

    let metrics = MetricsCollector::new();

    let build_ctx = |status: QueryStatus, idx: usize| {
        let mut req = Message::query();
        req.add_query(hickory_proto::op::Query::query(
            Name::from_str(&format!("host{}.example.com.", idx)).unwrap(),
            RecordType::A,
        ));
        let mut ctx = QueryContext::new("127.0.0.1".parse().unwrap(), req);
        ctx.status = status;
        ctx.finish();
        ctx
    };

    // 2 次缓存命中
    for i in 0..2 {
        metrics.record(&build_ctx(QueryStatus::Cached, i));
    }
    // 3 次回源未命中
    for i in 10..13 {
        metrics.record(&build_ctx(QueryStatus::Allowed, i));
    }
    // 5 次规则拦截 + 4 次本地重写: 均在缓存之前短路，不应进入缓存统计
    for i in 20..25 {
        metrics.record(&build_ctx(QueryStatus::Blocked, i));
    }
    for i in 30..34 {
        metrics.record(&build_ctx(QueryStatus::Rewritten, i));
    }

    let snap = metrics.snapshot();
    assert_eq!(snap.total_queries, 14);
    assert_eq!(snap.cached_queries, 2);
    // 分母只含 2 命中 + 3 回源 = 5
    assert!((snap.cache_hit_rate - 40.0).abs() < 1e-9, "hit rate was {}", snap.cache_hit_rate);

    let cache = metrics.cache_stats.snapshot();
    assert_eq!(cache.today.requests, 5);
    assert_eq!(cache.today.hits, 2);
    assert_eq!(cache.today.misses, 3);
    assert!((cache.today.hit_rate - 40.0).abs() < 1e-9);
    assert_eq!(cache.last_24h.requests, 5);
    assert_eq!(cache.last_24h.hits, 2);
    assert_eq!(cache.last_24h.misses, 3);
    assert!((cache.last_24h.hit_rate - 40.0).abs() < 1e-9);
}

#[test]
fn test_metrics_24h_scoped_reset() {
    use hickory_proto::op::Message;
    use nexdns::metrics::MetricsCollector;
    use nexdns::pipeline::context::{QueryContext, QueryStatus};

    let metrics = MetricsCollector::new();

    let record_one = |domain: &str, status: QueryStatus| {
        let mut req = Message::query();
        req.add_query(hickory_proto::op::Query::query(
            Name::from_str(&format!("{}.", domain)).unwrap(),
            RecordType::A,
        ));
        let mut ctx = QueryContext::new("10.0.0.8".parse().unwrap(), req);
        ctx.status = status;
        ctx.duration_ms = 12.0;
        metrics.record(&ctx);
    };

    record_one("good.example.com", QueryStatus::Allowed);
    record_one("good.example.com", QueryStatus::Cached);
    record_one("ad.example.com", QueryStatus::Blocked);

    let s1 = metrics.snapshot();
    assert_eq!(s1.total_queries, 3);
    assert_eq!(s1.blocked_queries, 1);
    assert_eq!(s1.cached_queries, 1);
    assert_eq!(s1.top_domains.len(), 1);
    assert_eq!(s1.top_blocked.len(), 1);
    assert_eq!(s1.top_clients.len(), 1);

    // Reset top_domains only
    metrics.reset("top_domains");
    let s2 = metrics.snapshot();
    assert!(s2.top_domains.is_empty());
    assert_eq!(s2.top_blocked.len(), 1);
    assert_eq!(s2.total_queries, 3);

    // Reset kpi_blocked only
    metrics.reset("kpi_blocked");
    let s3 = metrics.snapshot();
    assert_eq!(s3.blocked_queries, 0);
    assert_eq!(s3.block_rate, 0.0);
    assert_eq!(s3.total_queries, 3);
    assert_eq!(s3.cached_queries, 1);

    // Reset all
    metrics.reset("all");
    let s4 = metrics.snapshot();
    assert_eq!(s4.total_queries, 0);
    assert_eq!(s4.cached_queries, 0);
    assert!(s4.top_blocked.is_empty());
    assert!(s4.top_clients.is_empty());
    assert_eq!(metrics.cache_stats.snapshot().last_24h.requests, 0);
}

#[test]
fn test_udp_truncation_and_safe_mtu_limit() {
    use hickory_proto::op::{Message, MessageType, OpCode, Query};
    use hickory_proto::rr::{RData, Record};
    use nexdns::server::encode_udp_response;

    let mut resp = Message::new(0x1234, MessageType::Response, OpCode::Query);
    let qname = Name::from_str("cdn.heavy-records.example.com.").unwrap();
    resp.add_query(Query::query(qname.clone(), RecordType::A));

    for i in 0..80u8 {
        resp.add_answer(Record::from_rdata(
            qname.clone(),
            60,
            RData::A(hickory_proto::rr::rdata::A::new(10, 0, 0, i)),
        ));
    }

    // 1. 无 EDNS 客户端：限制在 512 字节以内并标记 TC=1
    let legacy_bytes = encode_udp_response(&resp, None).unwrap();
    assert!(legacy_bytes.len() <= 512, "legacy UDP size was {}", legacy_bytes.len());
    let legacy_decoded = Message::from_vec(&legacy_bytes).unwrap();
    assert!(legacy_decoded.metadata.truncation);

    // 2. 携带 EDNS 4096 的客户端：限制在 1232 字节安全 MTU 以内并标记 TC=1
    let edns_bytes = encode_udp_response(&resp, Some(4096)).unwrap();
    assert!(edns_bytes.len() <= 1232, "EDNS UDP size was {}", edns_bytes.len());
    let edns_decoded = Message::from_vec(&edns_bytes).unwrap();
    assert!(edns_decoded.metadata.truncation);
}

#[tokio::test]
async fn test_response_mode_ipv4_only_short_circuits_and_filters_cache() {
    use std::net::Ipv6Addr;
    use std::sync::Arc;
    use hickory_proto::op::{MessageType, OpCode, Query, ResponseCode};
    use hickory_proto::rr::rdata::AAAA;
    use nexdns::pipeline::context::QueryContext;
    use nexdns::pipeline::engine::PipelineEngine;
    use nexdns::pipeline::plugins::adblock::{AdblockPlugin, BlockType};
    use nexdns::pipeline::plugins::cache::CachePlugin;
    use nexdns::pipeline::plugins::domain_match::DomainListRegistry;
    use nexdns::pipeline::plugins::ip_match::IpListRegistry;
    use nexdns::pipeline::plugins::rewrite::RewritePlugin;
    use nexdns::pipeline::plugins::speed_check::SpeedCheckPlugin;
    use nexdns::pipeline::plugins::upstream::{ForwardPlugin, UpstreamGroup};

    let cache = Arc::new(CachePlugin::new(100, false, 60, 300));
    let qname = Name::from_str("dualstack.example.com.").unwrap();

    // 预置一条同时含 A 和 AAAA 的缓存响应，验证缓存命中路径同样执行 ipv4_only 过滤
    let mut cached_msg = Message::new(1, MessageType::Response, OpCode::Query);
    cached_msg.metadata.response_code = ResponseCode::NoError;
    cached_msg.add_query(Query::query(qname.clone(), RecordType::A));
    cached_msg.add_answer(Record::from_rdata(
        qname.clone(),
        60,
        RData::A(A(Ipv4Addr::new(1, 2, 3, 4))),
    ));
    cached_msg.add_answer(Record::from_rdata(
        qname.clone(),
        60,
        RData::AAAA(AAAA(Ipv6Addr::new(0x2400, 0x3200, 0, 0, 0, 0, 0, 1))),
    ));
    cache.insert("dualstack.example.com", RecordType::A, cached_msg.clone());
    cache.insert("dualstack.example.com", RecordType::AAAA, cached_msg);

    let engine = PipelineEngine::new(
        Arc::new(RewritePlugin::new()),
        Arc::new(AdblockPlugin::new(BlockType::ZeroIp, 60)),
        cache,
        Arc::new(DomainListRegistry::new()),
        Arc::new(IpListRegistry::new()),
        Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new("mainland", vec![], 500)))),
        Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new("gfw", vec![], 500)))),
        Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new("overseas", vec![], 500)))),
        Arc::new(SpeedCheckPlugin::new(false, 200, 5)),
        false,
        false,
        "ipv4_only".to_string(),
    );

    // 1. AAAA 查询在 ipv4_only 下前置短路为空 NoError (NODATA)，绝不返回缓存里的 AAAA
    let mut req_aaaa = Message::query();
    req_aaaa.add_query(Query::query(qname.clone(), RecordType::AAAA));
    let mut ctx_aaaa = QueryContext::new("127.0.0.1".parse().unwrap(), req_aaaa);
    engine.process(&mut ctx_aaaa).await;
    let resp_aaaa = ctx_aaaa.response.unwrap();
    assert_eq!(resp_aaaa.metadata.response_code, ResponseCode::NoError);
    assert!(resp_aaaa.answers.is_empty(), "ipv4_only 下 AAAA 查询应返回空 Answer");

    // 2. A 查询命中缓存时自动剥离混入的 AAAA 记录，只保留 A 记录
    let mut req_a = Message::query();
    req_a.add_query(Query::query(qname, RecordType::A));
    let mut ctx_a = QueryContext::new("127.0.0.1".parse().unwrap(), req_a);
    engine.process(&mut ctx_a).await;
    let resp_a = ctx_a.response.unwrap();
    assert_eq!(resp_a.answers.len(), 1);
    assert!(matches!(resp_a.answers[0].data, RData::A(_)));
}

#[tokio::test]
async fn test_proxy_route_fallback_to_mainland_when_gfw_unreachable() {
    use std::sync::Arc;
    use tokio::net::UdpSocket;
    use hickory_proto::op::{MessageType, OpCode, Query, ResponseCode};
    use nexdns::pipeline::context::QueryContext;
    use nexdns::pipeline::engine::PipelineEngine;
    use nexdns::pipeline::plugins::adblock::{AdblockPlugin, BlockType};
    use nexdns::pipeline::plugins::cache::CachePlugin;
    use nexdns::pipeline::plugins::domain_match::{DomainListRegistry, DomainMatcher};
    use nexdns::pipeline::plugins::ip_match::IpListRegistry;
    use nexdns::pipeline::plugins::rewrite::RewritePlugin;
    use nexdns::pipeline::plugins::speed_check::SpeedCheckPlugin;
    use nexdns::pipeline::plugins::upstream::{ForwardPlugin, UpstreamGroup, UpstreamServer};

    // 启动本地模拟大陆 UDP DNS 上游
    let mock_mainland = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mainland_addr = mock_mainland.local_addr().unwrap();
    tokio::spawn(async move {
        let mut buf = [0u8; 512];
        loop {
            if let Ok((len, src)) = mock_mainland.recv_from(&mut buf).await {
                if let Ok(req) = Message::from_vec(&buf[..len]) {
                    let mut resp = Message::new(req.metadata.id, MessageType::Response, OpCode::Query);
                    resp.metadata.response_code = ResponseCode::NoError;
                    resp.queries = req.queries.clone();
                    if let Some(q) = req.queries.first() {
                        resp.add_answer(Record::from_rdata(
                            q.name().clone(),
                            60,
                            RData::A(A(Ipv4Addr::new(104, 16, 1, 1))),
                        ));
                    }
                    let _ = mock_mainland.send_to(&resp.to_vec().unwrap(), src).await;
                }
            }
        }
    });

    let domain_reg = Arc::new(DomainListRegistry::new());
    let mut proxy_matcher = DomainMatcher::new("domain_proxy");
    proxy_matcher.load_from_text("github.com");
    domain_reg.register(proxy_matcher);

    let mainland_srv = UpstreamServer::parse("mainland", &format!("udp://{}", mainland_addr)).unwrap();
    // 指向未监听的本地 TCP 端口模拟境外上游不可达（立即 Connection refused）
    let dead_gfw = UpstreamServer::parse("gfw", "tcp://127.0.0.1:1").unwrap();
    let dead_overseas = UpstreamServer::parse("overseas", "tcp://127.0.0.1:2").unwrap();

    let engine = PipelineEngine::new(
        Arc::new(RewritePlugin::new()),
        Arc::new(AdblockPlugin::new(BlockType::ZeroIp, 60)),
        Arc::new(CachePlugin::new(100, false, 60, 300)),
        domain_reg,
        Arc::new(IpListRegistry::new()),
        Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new("mainland", vec![mainland_srv], 300)))),
        Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new("gfw", vec![dead_gfw], 300)))),
        Arc::new(ForwardPlugin::new(Arc::new(UpstreamGroup::new("overseas", vec![dead_overseas], 300)))),
        Arc::new(SpeedCheckPlugin::new(false, 200, 5)),
        true,
        true,
        "dual".to_string(),
    );

    let mut req = Message::query();
    req.add_query(Query::query(Name::from_str("github.com.").unwrap(), RecordType::A));
    let mut ctx = QueryContext::new("127.0.0.1".parse().unwrap(), req);
    engine.process(&mut ctx).await;

    let resp = ctx.response.expect("response should exist");
    assert_eq!(resp.metadata.response_code, ResponseCode::NoError);
    assert_eq!(resp.answers.len(), 1);
    assert!(ctx.tags.contains("fallback_restore_mainland"));
}

#[tokio::test]
async fn test_dual_stack_tcp_listener_bind_no_conflict() {
    use std::net::SocketAddr;
    use nexdns::server::bind_tcp_listener;

    let v4_addr: SocketAddr = "0.0.0.0:0".parse().unwrap();
    let v4_listener = bind_tcp_listener(v4_addr).expect("bind IPv4 TCP listener should succeed");
    let port = v4_listener.local_addr().unwrap().port();

    let v6_addr: SocketAddr = format!("[::]:{}", port).parse().unwrap();
    if let Ok(v6_listener) = bind_tcp_listener(v6_addr) {
        assert_eq!(v6_listener.local_addr().unwrap().port(), port);
    }
}
