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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub socks5: Option<String>,
    #[serde(default)]
    pub list: Vec<ProxyItem>,
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
            socks5: None,
            list: Vec::new(),
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
            max_ttl: 1800,
        }
    }
}

fn default_min_ttl() -> u32 {
    60
}

fn default_max_ttl() -> u32 {
    1800
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpeedCheckConfig {
    pub enabled: bool,
    pub timeout_ms: u64,
    pub cache_mins: u64,
    #[serde(default = "default_speed_mode")]
    pub mode: String,
    #[serde(default = "default_true")]
    pub direct_only: bool,
}

impl Default for SpeedCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 400,
            cache_mins: 5,
            mode: "tcp_80_443".to_string(),
            direct_only: true,
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
        "https://218.30.118.6/dns-query#sni=doh.360.cn".to_string(),
    ]
}

impl Default for UpstreamsConfig {
    fn default() -> Self {
        Self {
            bootstrap_dns: default_bootstrap_dns(),
            mainland: vec![
                "https://1.12.12.12/dns-query#sni=doh.pub".to_string(),
                "https://101.226.4.6/dns-query#sni=doh.360.cn".to_string(),
                "https://223.5.5.5/dns-query#sni=dns.alidns.com".to_string(),
            ],
            gfw: vec![
                "https://8.8.8.8/dns-query#sni=dns.google&proxy=default".to_string(),
                "https://8.8.4.4/dns-query#sni=dns.google&proxy=default".to_string(),
                "https://1.1.1.1/dns-query#proxy=default".to_string(),
                "https://194.242.2.2/dns-query#sni=dns.mullvad.net&proxy=default".to_string(),
            ],
            // 海外无代理防污染直连备用组（全部经纯国内宽带 lan-108 实机 HTTP/2 POST 报文拨测验证 100% 直连通达、无污染返回真实境外 IP）
            overseas: vec![
                "https://194.242.2.2/dns-query#sni=dns.mullvad.net&proxy=none".to_string(),
                "https://194.242.2.4/dns-query#sni=base.dns.mullvad.net&proxy=none".to_string(),
                "https://158.64.1.29/dns-query#sni=kaitain.restena.lu&proxy=none".to_string(),
                "https://185.228.169.9/doh/security-filter/#sni=doh.cleanbrowsing.org&proxy=none".to_string(),
                "https://185.95.218.43/dns-query#sni=dns.digitale-gesellschaft.ch&proxy=none".to_string(),
                "https://185.95.218.42/dns-query#sni=dns.digitale-gesellschaft.ch&proxy=none".to_string(),
            ],
            timeout_ms: 4000,
        }
    }
}

fn default_fallback_timeout_ms() -> u64 {
    800
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
    /// 智能分流与非中国 IP 防污染引擎总开关（合二为一）
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 兼容旧版独立 anti_pollution 字段反序列化，运行时已合并入智能分流引擎
    #[serde(default, skip_serializing)]
    pub anti_pollution: Option<bool>,
    #[serde(default = "default_fallback_timeout_ms")]
    pub fallback_timeout_ms: u64,
    #[serde(default)]
    pub custom_direct_domains: Vec<String>,
    #[serde(default)]
    pub custom_proxy_domains: Vec<String>,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            anti_pollution: None,
            fallback_timeout_ms: 800,
            custom_direct_domains: Vec::new(),
            custom_proxy_domains: Vec::new(),
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
    /// 兼容旧版顶层 anti_pollution 字段：启动归一化时自动迁移至 routing.anti_pollution 并不再重复序列化
    #[serde(default, skip_serializing)]
    pub anti_pollution: Option<bool>,
    /// 兼容顶层直接书写的 custom_direct_domains / custom_proxy_domains，归一化后统一归入 routing
    #[serde(default, skip_serializing)]
    pub custom_direct_domains: Vec<String>,
    #[serde(default, skip_serializing)]
    pub custom_proxy_domains: Vec<String>,
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
            speed_check: SpeedCheckConfig::default(),
            anti_pollution: None,
            custom_direct_domains: Vec::new(),
            custom_proxy_domains: Vec::new(),
            routing: RoutingConfig::default(),
            logging: LoggingConfig::default(),
            rules_update: RulesUpdateConfig::default(),
            upstreams: UpstreamsConfig::default(),
            rewrites: Vec::new(),
            adblock_rules: Vec::new(),
            routing_rules: Vec::new(),
        }
    }
}

fn clean_domain_list(list: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    let mut cleaned = Vec::with_capacity(list.len());
    for item in list.drain(..) {
        for part in item.lines() {
            let trimmed = part.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let lower = trimmed.to_lowercase();
            if seen.insert(lower.clone()) {
                cleaned.push(lower);
            }
        }
    }
    *list = cleaned;
}

impl AppConfig {
    pub fn normalize(&mut self) {
        let legacy_top_ap = self.anti_pollution.take();
        let legacy_routing_ap = self.routing.anti_pollution.take();
        if legacy_top_ap == Some(true) || legacy_routing_ap == Some(true) {
            self.routing.enabled = true;
        }
        if !self.custom_direct_domains.is_empty() {
            let extra = std::mem::take(&mut self.custom_direct_domains);
            self.routing.custom_direct_domains.extend(extra);
        }
        if !self.custom_proxy_domains.is_empty() {
            let extra = std::mem::take(&mut self.custom_proxy_domains);
            self.routing.custom_proxy_domains.extend(extra);
        }
        clean_domain_list(&mut self.routing.custom_direct_domains);
        clean_domain_list(&mut self.routing.custom_proxy_domains);

        if self.routing.fallback_timeout_ms == 0 {
            self.routing.fallback_timeout_ms = default_fallback_timeout_ms();
        } else {
            self.routing.fallback_timeout_ms = self.routing.fallback_timeout_ms.clamp(100, 30000);
        }
        if self.speed_check.timeout_ms == 0 {
            self.speed_check.timeout_ms = 400;
        } else {
            self.speed_check.timeout_ms = self.speed_check.timeout_ms.clamp(50, 10000);
        }
        if self.speed_check.mode.trim().is_empty() {
            self.speed_check.mode = default_speed_mode();
        }

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
            self.upstreams.timeout_ms = 4000;
        }
        // 自动清理旧版默认 overseas 中在大陆纯直连环境下已被 TCP 黑洞或 TLS SNI 重置的失效公共节点，并无缝注入实测直连可用节点
        let is_dead_direct_overseas = |u: &str| -> bool {
            let is_direct = u.contains("proxy=none") || !u.contains("proxy=");
            if !is_direct {
                return false;
            }
            const DEAD_DIRECT_HOSTS: &[&str] = &[
                "://1.1.1.1/",
                "://1.0.0.1/",
                "://8.8.8.8/",
                "://8.8.4.4/",
                "://94.140.14.140/",
                "://94.140.14.141/",
                "://168.95.1.1/",
                "://168.95.192.1/",
                "://101.101.101.101/",
                "://9.9.9.11/",
                "://9.9.9.9/",
                "://9.9.9.10/",
                "://149.112.112.11/",
                "://149.112.112.112/",
                "://77.88.8.8/",
                "://77.88.8.1/",
                "://208.67.222.222/",
                "://208.67.220.220/",
            ];
            DEAD_DIRECT_HOSTS.iter().any(|pat| u.contains(pat))
        };
        let prev_len = self.upstreams.overseas.len();
        self.upstreams.overseas.retain(|u| !is_dead_direct_overseas(u));
        if self.upstreams.overseas.len() != prev_len || (prev_len > 0 && self.upstreams.overseas.is_empty()) {
            let mut merged = UpstreamsConfig::default().overseas;
            for custom in self.upstreams.overseas.drain(..) {
                if !merged.contains(&custom) {
                    merged.push(custom);
                }
            }
            self.upstreams.overseas = merged;
        }
        for u in self.upstreams.gfw.iter_mut().chain(self.upstreams.overseas.iter_mut()) {
            if u.contains("#sni=dns.adguard.com") {
                *u = u.replace("#sni=dns.adguard.com", "#sni=dns.adguard-dns.com");
            }
        }
        self.dns.listen_mode = match self.dns.listen_mode.trim().to_lowercase().as_str() {
            "ipv4" | "ipv4_only" => "ipv4".to_string(),
            "ipv6" | "ipv6_only" => "ipv6".to_string(),
            _ => "dual".to_string(),
        };
        self.dns.connect_ip_version = match self.dns.connect_ip_version.trim().to_lowercase().as_str() {
            "ipv4" | "ipv4_only" => "ipv4".to_string(),
            "ipv6" | "ipv6_only" => "ipv6".to_string(),
            _ => "dual".to_string(),
        };
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
            let _ = config.save(&path);
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
        cloned.anti_pollution = None;
        cloned.routing.anti_pollution = None;
        cloned.custom_direct_domains.clear();
        cloned.custom_proxy_domains.clear();
        let content = serde_yaml::to_string(&cloned)?;
        fs::write(path, content)?;
        Ok(())
    }
}
