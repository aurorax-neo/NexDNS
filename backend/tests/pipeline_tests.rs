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
