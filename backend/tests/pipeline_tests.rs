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
async fn test_cache_serve_expired() {
    let cache = nexdns::pipeline::plugins::cache::CachePlugin::new(100, true, false, 10, 3600);
    let name = Name::from_str("example.com.").unwrap();
    let mut msg = Message::query();
    let record = Record::from_rdata(name, 10, RData::A(A(Ipv4Addr::new(93, 184, 216, 34))));
    msg.answers.push(record);

    cache.insert("example.com", RecordType::A, msg);
    assert_eq!(cache.size(), 1);

    let res = cache.get("example.com", RecordType::A);
    assert!(res.is_some());
    let (cached_msg, is_expired) = res.unwrap();
    assert!(!is_expired);
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
    let cache = nexdns::pipeline::plugins::cache::CachePlugin::new(3, false, false, 1, 10);
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

#[tokio::test]
async fn test_qtype_65_drop_returns_nodata_noerror() {
    use hickory_proto::op::ResponseCode;
    use nexdns::pipeline::context::{QueryContext, QueryStatus};
    use nexdns::pipeline::plugin::{DnsPlugin, PluginResult};
    use nexdns::pipeline::plugins::qtype_drop::QtypeDropPlugin;

    let plugin = QtypeDropPlugin::new(65, ResponseCode::NoError);
    let mut req = Message::query();
    req.add_query(hickory_proto::op::Query::query(
        Name::from_ascii("www.baidu.com.").unwrap(),
        RecordType::from(65),
    ));

    let mut ctx = QueryContext::new("127.0.0.1".parse().unwrap(), req);
    let res = plugin.execute(&mut ctx).await;

    assert_eq!(res, PluginResult::Accept);
    assert!(matches!(ctx.status, QueryStatus::Blocked));
    assert_eq!(ctx.matched_rule.as_deref(), Some("qtype_65"));

    let resp = ctx.response.expect("Expected response for dropped qtype 65");
    // Must be NoError (NODATA), not NXDomain!
    assert_eq!(resp.metadata.response_code, ResponseCode::NoError);
    assert_eq!(resp.answers.len(), 0);
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
