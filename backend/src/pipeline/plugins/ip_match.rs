use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use ipnet::IpNet;
use parking_lot::RwLock;

#[derive(Default, Clone)]
pub struct IpMatcher {
    pub name: String,
    pub subnets: Vec<IpNet>,
}

impl IpMatcher {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            subnets: Vec::new(),
        }
    }

    pub fn load_from_text(&mut self, text: &str) {
        for line in text.lines() {
            let mut trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("cidr:") {
                trimmed = rest.trim();
            }
            if let Ok(net) = IpNet::from_str(trimmed) {
                self.subnets.push(net);
            } else if let Ok(ip) = IpAddr::from_str(trimmed) {
                self.subnets.push(IpNet::from(ip));
            }
        }
    }

    pub fn contains(&self, ip: &IpAddr) -> bool {
        for net in &self.subnets {
            if net.contains(ip) {
                return true;
            }
        }
        false
    }
}

pub struct IpListRegistry {
    matchers: Arc<RwLock<HashMap<String, IpMatcher>>>,
}

impl IpListRegistry {
    pub fn new() -> Self {
        let mut reg = HashMap::new();
        
        // Default private IP subnets
        let mut private = IpMatcher::new("private");
        private.load_from_text(
            "10.0.0.0/8\n172.16.0.0/12\n192.168.0.0/16\n127.0.0.0/8\n100.64.0.0/10\n169.254.0.0/16\nfc00::/7\nfe80::/10\n::1/128"
        );
        reg.insert("private".to_string(), private);

        // Default China IP representative major ranges
        let mut china = IpMatcher::new("ipv4_china");
        china.load_from_text(
            "1.0.1.0/24\n1.0.2.0/23\n1.0.8.0/21\n1.0.32.0/19\n14.0.0.0/10\n27.0.0.0/10\n36.0.0.0/10\n39.0.0.0/10\n42.0.0.0/10\n49.0.0.0/10\n58.0.0.0/10\n59.0.0.0/10\n60.0.0.0/10\n61.0.0.0/10\n101.0.0.0/10\n106.0.0.0/10\n110.0.0.0/10\n111.0.0.0/10\n112.0.0.0/10\n113.0.0.0/10\n114.0.0.0/10\n115.0.0.0/10\n116.0.0.0/10\n117.0.0.0/10\n118.0.0.0/10\n119.0.0.0/10\n120.0.0.0/10\n121.0.0.0/10\n122.0.0.0/10\n123.0.0.0/10\n124.0.0.0/10\n125.0.0.0/10\n180.0.0.0/10\n182.0.0.0/10\n183.0.0.0/10\n218.0.0.0/10\n219.0.0.0/10\n220.0.0.0/10\n221.0.0.0/10\n222.0.0.0/10\n223.0.0.0/10"
        );
        reg.insert("ipv4_china".to_string(), china);

        Self {
            matchers: Arc::new(RwLock::new(reg)),
        }
    }

    pub fn register(&self, matcher: IpMatcher) {
        let mut w = self.matchers.write();
        w.insert(matcher.name.clone(), matcher);
    }

    pub fn contains(&self, list_name: &str, ip: &IpAddr) -> bool {
        let r = self.matchers.read();
        if let Some(matcher) = r.get(list_name) {
            matcher.contains(ip)
        } else {
            false
        }
    }
}
