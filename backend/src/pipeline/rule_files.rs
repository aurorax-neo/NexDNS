use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::config::AppConfig;
use crate::pipeline::plugins::domain_match::{DomainListRegistry, DomainMatcher};
use crate::pipeline::plugins::ip_match::{IpListRegistry, IpMatcher};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuleFileInfo {
    pub id: String,
    pub name: String,
    pub file_name: String,
    pub group_type: String, // "mainland", "gfw", "private", "china_ip"
    pub count: usize,
    pub size_bytes: u64,
    pub updated_at: String,
    pub url: String,
    pub enabled: bool,
}

pub struct RuleFileManager {
    base_dir: PathBuf,
    config: Arc<parking_lot::RwLock<AppConfig>>,
    config_path: String,
    domain_registry: Arc<DomainListRegistry>,
    ip_registry: Arc<IpListRegistry>,
}

impl RuleFileManager {
    pub fn new<P: AsRef<Path>>(
        base_dir: P,
        config: Arc<parking_lot::RwLock<AppConfig>>,
        config_path: String,
        domain_registry: Arc<DomainListRegistry>,
        ip_registry: Arc<IpListRegistry>,
    ) -> Self {
        let p = base_dir.as_ref().to_path_buf();
        let _ = fs::create_dir_all(&p);
        let s = Self {
            base_dir: p,
            config,
            config_path,
            domain_registry,
            ip_registry,
        };
        s.ensure_defaults();
        s
    }

    pub fn rules_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn ensure_defaults(&self) {
        let defaults = vec![
            ("大陆直连主规则 (direct.txt)", "direct.txt", "mainland", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/direct.txt"),
            ("代理过滤主规则 (proxy.txt)", "proxy.txt", "gfw", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/proxy.txt"),
            ("局域私有域名 (private.txt)", "private.txt", "private", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/private.txt"),
            ("中国 IPv4 地址段 (ipv4_china.txt)", "ipv4_china.txt", "china_ip", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv4_china.txt"),
            ("中国 IPv6 地址段 (ipv6_china.txt)", "ipv6_china.txt", "china_ip", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv6_china.txt"),
        ];

        let mut cfg = self.config.write();
        let mut changed = false;

        for (name, file_name, group_type, url) in defaults {
            let exists = cfg.routing_rules.iter().any(|r| r.file_name == file_name);
            if !exists {
                let file_path = self.base_dir.join(file_name);
                let size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                let line_count = fs::read_to_string(&file_path)
                    .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
                    .unwrap_or(0);

                cfg.routing_rules.push(RuleFileInfo {
                    id: Uuid::new_v4().to_string(),
                    name: name.to_string(),
                    file_name: file_name.to_string(),
                    group_type: group_type.to_string(),
                    url: url.to_string(),
                    enabled: true,
                    count: line_count,
                    size_bytes: size,
                    updated_at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
                });
                changed = true;
            }
        }

        if changed {
            let _ = cfg.save(&self.config_path);
        }
    }

    pub fn get_disk_fallback_rules(&self) -> Vec<RuleFileInfo> {
        let defaults = vec![
            ("大陆直连主规则 (direct.txt)", "direct.txt", "mainland", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/direct.txt"),
            ("代理过滤主规则 (proxy.txt)", "proxy.txt", "gfw", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/proxy.txt"),
            ("局域私有域名 (private.txt)", "private.txt", "private", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/private.txt"),
            ("中国 IPv4 地址段 (ipv4_china.txt)", "ipv4_china.txt", "china_ip", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv4_china.txt"),
            ("中国 IPv6 地址段 (ipv6_china.txt)", "ipv6_china.txt", "china_ip", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv6_china.txt"),
        ];

        let mut res = Vec::new();
        for (name, file_name, group_type, url) in defaults {
            let file_path = self.base_dir.join(file_name);
            let size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
            let line_count = fs::read_to_string(&file_path)
                .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
                .unwrap_or(0);

            res.push(RuleFileInfo {
                id: format!("builtin_{}", file_name.replace('.', "_")),
                name: name.to_string(),
                file_name: file_name.to_string(),
                group_type: group_type.to_string(),
                url: url.to_string(),
                enabled: true,
                count: line_count,
                size_bytes: size,
                updated_at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
            });
        }
        res
    }

    pub fn list_rules(&self) -> Vec<RuleFileInfo> {
        self.ensure_defaults();
        let mut result = self.config.read().routing_rules.clone();
        if result.is_empty() {
            return self.get_disk_fallback_rules();
        }
        result.sort_by(|a, b| a.group_type.cmp(&b.group_type).then_with(|| a.name.cmp(&b.name)));
        result
    }

    pub fn add_rule_file(&self, name: &str, file_name: &str, group_type: &str, url: &str, content: &str) -> anyhow::Result<String> {
        let mut safe_name = name.to_string();
        let mut safe_file_name = file_name.to_string();
        let p = std::path::Path::new(file_name);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("txt");

        let mut idx = 1;
        while self.base_dir.join(&safe_file_name).exists() || {
            let cfg = self.config.read();
            cfg.routing_rules.iter().any(|r| r.file_name == safe_file_name)
        } {
            safe_file_name = format!("{}_{}.{}", stem, idx, ext);
            safe_name = format!("{} ({})", name, idx);
            idx += 1;
        }

        let file_path = self.base_dir.join(&safe_file_name);
        if !content.is_empty() {
            fs::write(&file_path, content)?;
        } else if !file_path.exists() {
            fs::write(&file_path, "")?;
        }

        let size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        let line_count = fs::read_to_string(&file_path)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
            .unwrap_or(0);

        let id = Uuid::new_v4().to_string();
        {
            let mut cfg = self.config.write();
            cfg.routing_rules.push(RuleFileInfo {
                id,
                name: safe_name,
                file_name: safe_file_name.clone(),
                group_type: group_type.to_string(),
                url: url.to_string(),
                enabled: true,
                count: line_count,
                size_bytes: size,
                updated_at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
            });
            cfg.save(&self.config_path)?;
        }

        self.load_local_rules();
        Ok(safe_file_name)
    }

    pub fn toggle_rule_file(&self, id: &str) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            if let Some(item) = cfg.routing_rules.iter_mut().find(|r| r.id == id) {
                item.enabled = !item.enabled;
            }
            cfg.save(&self.config_path)?;
        }
        self.load_local_rules();
        Ok(())
    }

    pub fn edit_rule_info(&self, id: &str, name: &str, group_type: &str, url: &str) -> anyhow::Result<()> {
        {
            let mut cfg = self.config.write();
            if let Some(item) = cfg.routing_rules.iter_mut().find(|r| r.id == id) {
                item.name = name.to_string();
                item.group_type = group_type.to_string();
                item.url = url.to_string();
            }
            cfg.save(&self.config_path)?;
        }
        self.load_local_rules();
        Ok(())
    }

    pub fn delete_rule_file(&self, id: &str) -> anyhow::Result<()> {
        let file_name = {
            let mut cfg = self.config.write();
            let pos = cfg
                .routing_rules
                .iter()
                .position(|r| r.id == id)
                .ok_or_else(|| anyhow::anyhow!("未找到指定的规则文件"))?;
            let removed = cfg.routing_rules.remove(pos);
            cfg.save(&self.config_path)?;
            removed.file_name
        };
        let _ = fs::remove_file(self.base_dir.join(file_name));
        self.load_local_rules();
        Ok(())
    }

    pub fn refresh_file_stats(&self, file_name: &str) {
        let file_path = self.base_dir.join(file_name);
        let size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
        let line_count = fs::read_to_string(&file_path)
            .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
            .unwrap_or(0);
        let mut cfg = self.config.write();
        if let Some(item) = cfg.routing_rules.iter_mut().find(|r| r.file_name == file_name) {
            item.count = line_count;
            item.size_bytes = size;
            item.updated_at = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
            let _ = cfg.save(&self.config_path);
        }
    }

    pub async fn update_rule_file(&self, file_name: &str, url: &str) -> anyhow::Result<usize> {
        let resp = reqwest::get(url).await?;
        let text = resp.text().await?;

        let file_path = self.base_dir.join(file_name);
        fs::write(&file_path, &text)?;

        let line_count = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .count();
        let size = text.len() as u64;

        {
            let mut cfg = self.config.write();
            if let Some(item) = cfg.routing_rules.iter_mut().find(|r| r.file_name == file_name) {
                item.count = line_count;
                item.size_bytes = size;
                item.updated_at = chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
            }
            let _ = cfg.save(&self.config_path);
        }

        self.load_local_rules();
        Ok(line_count)
    }

    pub async fn update_all(&self) -> Vec<(String, Result<usize, String>)> {
        let files = self.list_rules();
        let mut res = Vec::new();
        for f in files {
            if !f.url.is_empty() && f.url.starts_with("http") {
                match self.update_rule_file(&f.file_name, &f.url).await {
                    Ok(cnt) => res.push((f.file_name, Ok(cnt))),
                    Err(e) => res.push((f.file_name, Err(e.to_string()))),
                }
            }
        }
        res
    }

    pub fn load_local_rules(&self) {
        let rules = self.list_rules();

        let mut direct_text = String::new();
        let mut proxy_text = String::new();
        let mut private_text = String::new();
        let mut china_ipv4_text = String::new();
        let mut china_ipv6_text = String::new();

        for r in rules {
            if !r.enabled {
                continue;
            }
            let p = self.base_dir.join(&r.file_name);
            if let Ok(content) = fs::read_to_string(&p) {
                match r.group_type.as_str() {
                    "mainland" => {
                        direct_text.push_str(&content);
                        direct_text.push('\n');
                    }
                    "gfw" | "proxy" => {
                        proxy_text.push_str(&content);
                        proxy_text.push('\n');
                    }
                    "private" => {
                        private_text.push_str(&content);
                        private_text.push('\n');
                    }
                    "china_ip" => {
                        if r.file_name.contains("ipv6") {
                            china_ipv6_text.push_str(&content);
                            china_ipv6_text.push('\n');
                        } else {
                            china_ipv4_text.push_str(&content);
                            china_ipv4_text.push('\n');
                        }
                    }
                    _ => {}
                }
            }
        }

        // Combine into domain and IP registries
        let mut direct_matcher = DomainMatcher::new("domain_direct");
        direct_matcher.load_from_text(&direct_text);
        self.domain_registry.register(direct_matcher);

        let mut proxy_matcher = DomainMatcher::new("domain_proxy");
        proxy_matcher.load_from_text(&proxy_text);
        self.domain_registry.register(proxy_matcher);

        let mut private_matcher = DomainMatcher::new("domain_private");
        private_matcher.load_from_text(&private_text);
        self.domain_registry.register(private_matcher);

        let mut china_ipv4_matcher = IpMatcher::new("ipv4_china");
        china_ipv4_matcher.load_from_text(&china_ipv4_text);
        self.ip_registry.register(china_ipv4_matcher);

        let mut china_ipv6_matcher = IpMatcher::new("ipv6_china");
        china_ipv6_matcher.load_from_text(&china_ipv6_text);
        self.ip_registry.register(china_ipv6_matcher);
    }
}
