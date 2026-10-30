use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyItem {
    pub id: String,
    pub name: String,
    pub addr: String,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub socks5: Option<String>,
    #[serde(default = "default_proxy_list")]
    pub list: Vec<ProxyItem>,
}

fn default_proxy_list() -> Vec<ProxyItem> {
    vec![
        ProxyItem {
            id: "local_clash".to_string(),
            name: "本地 Clash/Mihomo 节点".to_string(),
            addr: "socks5://127.0.0.1:7890".to_string(),
            is_default: true,
        }
    ]
}

impl ProxyConfig {
    pub fn default_proxy_addr(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        self.list
            .iter()
            .find(|p| p.is_default)
            .map(|p| p.addr.clone())
            .or_else(|| self.socks5.clone().filter(|s| !s.is_empty()))
    }

    pub fn find_proxy_addr(&self, id_or_mode: &str) -> Option<String> {
        let mode = id_or_mode.trim();
        if mode == "none" || mode.is_empty() {
            return None;
        }
        if !self.enabled {
            return None;
        }
        if mode == "default" {
            return self.default_proxy_addr();
        }
        if let Some(p) = self.list.iter().find(|p| p.id == mode || p.name == mode) {
            return Some(p.addr.clone());
        }
        if mode.contains("://") {
            return Some(mode.to_string());
        }
        self.default_proxy_addr()
    }
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            socks5: Some("socks5://127.0.0.1:7890".to_string()),
            list: default_proxy_list(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub port: u16,
    #[serde(default = "default_listen_mode")]
    pub listen_mode: String,
    #[serde(default = "default_listen_mode")]
    pub connect_ip_version: String,
    #[serde(default = "default_response_mode")]
    pub response_mode: String,
    #[serde(default = "default_true")]
    pub doh_enabled: bool,
    #[serde(default = "default_doh_port")]
    pub doh_port: u16,
    #[serde(default = "default_doh_path")]
    pub doh_path: String,
    #[serde(default = "default_block_type")]
    pub block_type: String,
    #[serde(default = "default_block_ttl")]
    pub block_ttl: u32,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 53,
            listen_mode: "dual".to_string(),
            connect_ip_version: "dual".to_string(),
            response_mode: "dual".to_string(),
            doh_enabled: true,
            doh_port: 8053,
            doh_path: "/dns-query".to_string(),
            block_type: "zero_ip".to_string(),
            block_ttl: 60,
        }
    }
}

fn default_doh_port() -> u16 {
    8053
}

fn default_true() -> bool {
    true
}

fn default_listen_mode() -> String {
    "dual".to_string()
}

fn default_response_mode() -> String {
    "dual".to_string()
}

fn default_doh_path() -> String {
    "/dns-query".to_string()
}

fn default_block_type() -> String {
    "zero_ip".to_string()
}

fn default_block_ttl() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self { port: 3000 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub size: usize,
    pub serve_expired: bool,
    pub prefetch: bool,
    #[serde(default = "default_min_ttl")]
    pub min_ttl: u32,
    #[serde(default = "default_max_ttl")]
    pub max_ttl: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            size: 10000,
            serve_expired: true,
            prefetch: true,
            min_ttl: 60,
            max_ttl: 300,
        }
    }
}

fn default_min_ttl() -> u32 {
    60
}

fn default_max_ttl() -> u32 {
    300
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedCheckConfig {
    pub enabled: bool,
    pub timeout_ms: u64,
    pub cache_mins: u64,
    #[serde(default = "default_speed_mode")]
    pub mode: String,
}

impl Default for SpeedCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 400,
            cache_mins: 5,
            mode: "tcp_80_443".to_string(),
        }
    }
}

fn default_speed_mode() -> String {
    "tcp_80_443".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamsConfig {
    #[serde(default = "default_bootstrap_dns")]
    pub bootstrap_dns: Vec<String>,
    pub mainland: Vec<String>,
    pub gfw: Vec<String>,
    pub overseas: Vec<String>,
    pub timeout_ms: u64,
}

fn default_bootstrap_dns() -> Vec<String> {
    vec![
        "https://223.5.5.5/dns-query#sni=dns.alidns.com".to_string(),
        "https://1.12.12.12/dns-query#sni=doh.pub".to_string(),
        "https://8.8.8.8/dns-query#sni=dns.google".to_string(),
        "https://1.1.1.1/dns-query".to_string(),
        "https://1.0.0.1/dns-query".to_string(),
        "223.5.5.5".to_string(),
        "119.29.29.29".to_string(),
    ]
}

impl Default for UpstreamsConfig {
    fn default() -> Self {
        Self {
            bootstrap_dns: default_bootstrap_dns(),
            mainland: vec![
                "udp://223.5.5.5:53".to_string(),
                "udp://119.29.29.29:53".to_string(),
                "https://dns.alidns.com/dns-query".to_string(),
            ],
            gfw: vec![
                "https://dns.google/dns-query".to_string(),
                "https://cloudflare-dns.com/dns-query".to_string(),
                "udp://8.8.8.8:53".to_string(),
            ],
            overseas: vec![
                "udp://1.1.1.1:53".to_string(),
                "https://1.1.1.1/dns-query".to_string(),
                "udp://8.8.4.4:53".to_string(),
            ],
            timeout_ms: 2000,
        }
    }
}

fn default_false() -> bool {
    false
}

fn default_retention_mins() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub anti_pollution: bool,
    #[serde(default = "default_false")]
    pub qtype_65_reject: bool,
    pub fallback_timeout_ms: u64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            anti_pollution: true,
            qtype_65_reject: false,
            fallback_timeout_ms: 1500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub enabled: bool,
    #[serde(default = "default_retention_mins")]
    pub retention_mins: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retention_mins: 60,
            retention_days: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesUpdateConfig {
    #[serde(default = "default_rules_auto_update")]
    pub auto_update: bool,
    #[serde(default = "default_rules_update_interval_mins")]
    pub interval_mins: u64,
}

fn default_rules_auto_update() -> bool {
    true
}

fn default_rules_update_interval_mins() -> u64 {
    1440
}

impl Default for RulesUpdateConfig {
    fn default() -> Self {
        Self {
            auto_update: true,
            interval_mins: 1440,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub dns: DnsConfig,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub speed_check: SpeedCheckConfig,
    #[serde(default)]
    pub anti_pollution: bool,
    #[serde(default)]
    pub routing: RoutingConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub rules_update: RulesUpdateConfig,
    #[serde(default)]
    pub upstreams: UpstreamsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dns: DnsConfig::default(),
            web: WebConfig { port: 3000 },
            proxy: ProxyConfig::default(),
            cache: CacheConfig {
                size: 10000,
                serve_expired: true,
                prefetch: true,
                min_ttl: 60,
                max_ttl: 300,
            },
            speed_check: SpeedCheckConfig {
                enabled: true,
                timeout_ms: 400,
                cache_mins: 5,
                mode: "tcp_80_443".to_string(),
            },
            anti_pollution: true,
            routing: RoutingConfig::default(),
            logging: LoggingConfig::default(),
            rules_update: RulesUpdateConfig::default(),
            upstreams: UpstreamsConfig {
                bootstrap_dns: vec![
                    "https://223.5.5.5/dns-query#sni=dns.alidns.com".to_string(),
                    "https://1.12.12.12/dns-query#sni=doh.pub".to_string(),
                    "https://8.8.8.8/dns-query#sni=dns.google".to_string(),
                    "https://1.1.1.1/dns-query".to_string(),
                    "https://1.0.0.1/dns-query".to_string(),
                    "223.5.5.5".to_string(),
                    "119.29.29.29".to_string(),
                ],
                mainland: vec![
                    "https://1.12.12.12/dns-query#sni=doh.pub".to_string(),
                    "https://120.53.53.53/dns-query#sni=doh.pub".to_string(),
                    "https://101.226.4.6/dns-query#sni=doh.360.cn".to_string(),
                    "https://218.30.118.6/dns-query#sni=doh.360.cn".to_string(),
                    "https://223.5.5.5/dns-query#sni=dns.alidns.com".to_string(),
                    "https://223.6.6.6/dns-query#sni=dns.alidns.com".to_string(),
                    "udp://223.5.5.5:53".to_string(),
                    "udp://119.29.29.29:53".to_string(),
                ],
                gfw: vec![
                    "https://8.8.8.8/dns-query#sni=dns.google&proxy=default".to_string(),
                    "https://8.8.4.4/dns-query#sni=dns.google&proxy=default".to_string(),
                    "https://94.140.14.140/dns-query#sni=dns.adguard.com&proxy=default".to_string(),
                    "https://94.140.14.141/dns-query#sni=dns.adguard.com&proxy=default".to_string(),
                    "https://route.aorz.kdns.fr/dns-query#proxy=default".to_string(),
                    "https://1.1.1.1/dns-query#proxy=default".to_string(),
                    "https://1.0.0.1/dns-query#proxy=default".to_string(),
                ],
                overseas: vec![
                    "https://1.1.1.1/dns-query#proxy=default".to_string(),
                    "https://1.0.0.1/dns-query#proxy=default".to_string(),
                    "https://8.8.8.8/dns-query#sni=dns.google&proxy=default".to_string(),
                    "https://168.95.1.1/dns-query#sni=dns.hinet.net&proxy=default".to_string(),
                    "https://168.95.192.1/dns-query#sni=dns.hinet.net&proxy=default".to_string(),
                    "https://77.88.8.8/dns-query#proxy=default".to_string(),
                    "https://77.88.8.1/dns-query#proxy=default".to_string(),
                    "https://route.aorz.kdns.fr/dns-query#proxy=default".to_string(),
                ],
                timeout_ms: 2000,
            },
        }
    }
}

impl AppConfig {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        if path.as_ref().exists() {
            let content = fs::read_to_string(&path)?;
            let mut config: Self = serde_yaml::from_str(&content)?;
            // 平滑兼容历史旧配置: 若存在 retention_days 且 retention_mins 为 0，换算为分钟
            if let Some(days) = config.logging.retention_days {
                if config.logging.retention_mins == 0 {
                    config.logging.retention_mins = days * 1440;
                }
            }
            Ok(config)
        } else {
            let default_cfg = Self::default();
            if let Some(parent) = path.as_ref().parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_yaml::to_string(&default_cfg)?;
            fs::write(&path, content)?;
            Ok(default_cfg)
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
