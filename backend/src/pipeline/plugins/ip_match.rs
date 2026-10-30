use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;
use ipnet::IpNet;
use parking_lot::RwLock;

#[derive(Default, Clone)]
pub struct IpMatcher {
    pub name: String,
    pub v4_ranges: Vec<(u32, u32)>,
    pub v6_subnets: Vec<ipnet::Ipv6Net>,
}

impl IpMatcher {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            v4_ranges: Vec::new(),
            v6_subnets: Vec::new(),
        }
    }

    pub fn load_from_text(&mut self, text: &str) {
        let mut raw_v4 = Vec::new();
        for line in text.lines() {
            let mut trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("cidr:") {
                trimmed = rest.trim();
            }
            if let Ok(net) = IpNet::from_str(trimmed) {
                match net {
                    IpNet::V4(v4) => {
                        let start = u32::from(v4.network());
                        let end = u32::from(v4.broadcast());
                        raw_v4.push((start, end));
                    }
                    IpNet::V6(v6) => {
                        self.v6_subnets.push(v6);
                    }
                }
            } else if let Ok(ip) = IpAddr::from_str(trimmed) {
                match ip {
                    IpAddr::V4(v4) => {
                        let val = u32::from(v4);
                        raw_v4.push((val, val));
                    }
                    IpAddr::V6(v6) => {
                        if let Ok(net) = ipnet::Ipv6Net::new(v6, 128) {
                            self.v6_subnets.push(net);
                        }
                    }
                }
            }
        }

        // 归并排序 IPv4 区间，将上万条 CIDR 压缩为紧凑区间列表，实现 O(log n) 二分极速匹配
        if !raw_v4.is_empty() {
            raw_v4.sort_unstable_by_key(|&(start, _)| start);
            let mut merged = Vec::with_capacity(raw_v4.len());
            let (mut cur_start, mut cur_end) = raw_v4[0];
            for &(start, end) in &raw_v4[1..] {
                if start <= cur_end.saturating_add(1) {
                    cur_end = cur_end.max(end);
                } else {
                    merged.push((cur_start, cur_end));
                    cur_start = start;
                    cur_end = end;
                }
            }
            merged.push((cur_start, cur_end));
            self.v4_ranges = merged;
        }
    }

    pub fn contains(&self, ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(v4) => {
                let target = u32::from(*v4);
                self.v4_ranges.binary_search_by(|&(start, end)| {
                    if target < start {
                        std::cmp::Ordering::Greater
                    } else if target > end {
                        std::cmp::Ordering::Less
                    } else {
                        std::cmp::Ordering::Equal
                    }
                }).is_ok()
            }
            IpAddr::V6(v6) => {
                for net in &self.v6_subnets {
                    if net.contains(v6) {
                        return true;
                    }
                }
                false
            }
        }
    }
}

pub struct IpListRegistry {
    matchers: Arc<RwLock<HashMap<String, IpMatcher>>>,
}

impl Default for IpListRegistry {
    fn default() -> Self {
        Self::new()
    }
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
