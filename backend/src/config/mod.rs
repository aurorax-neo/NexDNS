use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::db::{AdblockRuleRecord, DnsRewriteRecord};
use crate::pipeline::rule_files::RuleFileInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default = "default_admin_username")]
    pub username: String,
    #[serde(default = "default_admin_password", alias = "password_hash")]
    pub password: String,
}

fn default_admin_username() -> String {
    "admin".to_string()
}

fn default_admin_password() -> String {
    crate::db::auth::hash_password("admin1234").unwrap_or_else(|_| "admin1234".to_string())
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            username: default_admin_username(),
            password: default_admin_password(),
        }
    }
}

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
#[serde(default)]
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
#[serde(default)]
pub struct WebConfig {
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self { port: 3000 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    pub size: usize,
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
            prefetch: true,
            min_ttl: 60,
            max_ttl: 120,
        }
    }
}

fn default_min_ttl() -> u32 {
    60
}

fn default_max_ttl() -> u32 {
    120
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
#[serde(default)]
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

fn default_max_disk_usage_percent() -> u8 {
    90
}

fn default_min_free_disk_mb() -> u64 {
    512
}

fn default_max_db_size_mb() -> u64 {
    512
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingConfig {
    #[serde(default = "default_false")]
    pub enabled: bool,
    pub anti_pollution: bool,
    pub fallback_timeout_ms: u64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            anti_pollution: true,
            fallback_timeout_ms: 1500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub retention_mins: u32,
    #[serde(default, skip_serializing)]
    pub retention_days: Option<u32>,
    #[serde(default = "default_max_disk_usage_percent")]
    pub max_disk_usage_percent: u8,
    #[serde(default = "default_min_free_disk_mb")]
    pub min_free_disk_mb: u64,
    #[serde(default = "default_max_db_size_mb")]
    pub max_db_size_mb: u64,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_mins: default_retention_mins(),
            retention_days: None,
            max_disk_usage_percent: default_max_disk_usage_percent(),
            min_free_disk_mb: default_min_free_disk_mb(),
            max_db_size_mb: default_max_db_size_mb(),
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
    pub auth: AuthConfig,
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
    #[serde(default)]
    pub rewrites: Vec<DnsRewriteRecord>,
    #[serde(default)]
    pub adblock_rules: Vec<AdblockRuleRecord>,
    #[serde(default)]
    pub routing_rules: Vec<RuleFileInfo>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            auth: AuthConfig::default(),
            dns: DnsConfig::default(),
            web: WebConfig { port: 3000 },
            proxy: ProxyConfig::default(),
            cache: CacheConfig::default(),
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
            rewrites: Vec::new(),
            adblock_rules: Vec::new(),
            routing_rules: Vec::new(),
        }
    }
}

impl AppConfig {
    pub fn normalize(&mut self) {
        if let Some(days) = self.logging.retention_days.take() {
            if self.logging.retention_mins == 0 {
                self.logging.retention_mins = (days * 1440).max(1);
            }
        }
        if self.logging.retention_mins == 0 {
            self.logging.retention_mins = default_retention_mins();
        }
        self.logging.max_disk_usage_percent = self.logging.max_disk_usage_percent.clamp(50, 98);
        if self.logging.min_free_disk_mb == 0 {
            self.logging.min_free_disk_mb = default_min_free_disk_mb();
        }
        if self.logging.max_db_size_mb == 0 {
            self.logging.max_db_size_mb = default_max_db_size_mb();
        }
        if self.upstreams.timeout_ms == 0 {
            self.upstreams.timeout_ms = 2000;
        }
        self.dns.response_mode = match self.dns.response_mode.trim() {
            "ipv4" | "ipv4_only" => "ipv4_only".to_string(),
            "ipv6" | "ipv6_only" => "ipv6_only".to_string(),
            _ => "dual".to_string(),
        };
        if self.auth.username.trim().is_empty() {
            self.auth.username = default_admin_username();
        }
        if self.auth.password.trim().is_empty() {
            self.auth.password = default_admin_password();
        } else if !self.auth.password.starts_with("$argon2") {
            if let Ok(hashed) = crate::db::auth::hash_password(self.auth.password.trim()) {
                self.auth.password = hashed;
            }
        }
    }

    pub fn load_or_create<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        if path.as_ref().exists() {
            let content = fs::read_to_string(&path)?;
            let mut config: Self = serde_yaml::from_str(&content)?;
            config.normalize();
            Ok(config)
        } else {
            let mut default_cfg = Self::default();
            default_cfg.normalize();
            if let Some(parent) = path.as_ref().parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_yaml::to_string(&default_cfg)?;
            fs::write(&path, content)?;
            Ok(default_cfg)
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let mut cloned = self.clone();
        cloned.logging.retention_days = None;
        let content = serde_yaml::to_string(&cloned)?;
        fs::write(path, content)?;
        Ok(())
    }
}
