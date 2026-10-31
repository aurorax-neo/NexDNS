use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
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
    db: Arc<crate::db::Database>,
    domain_registry: Arc<DomainListRegistry>,
    ip_registry: Arc<IpListRegistry>,
}

impl RuleFileManager {
    pub fn new<P: AsRef<Path>>(
        base_dir: P,
        db: Arc<crate::db::Database>,
        domain_registry: Arc<DomainListRegistry>,
        ip_registry: Arc<IpListRegistry>,
    ) -> Self {
        let p = base_dir.as_ref().to_path_buf();
        let _ = fs::create_dir_all(&p);
        let s = Self {
            base_dir: p,
            db,
            domain_registry,
            ip_registry,
        };
        let _ = s.init_db();
        s
    }

    pub fn rules_dir(&self) -> &Path {
        &self.base_dir
    }

    fn get_conn(&self) -> rusqlite::Result<crate::db::PooledConn<'_>> {
        self.db.get_conn()
    }

    fn init_db(&self) -> rusqlite::Result<()> {
        let conn = self.get_conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS routing_rule_files (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                file_name TEXT NOT NULL UNIQUE,
                group_type TEXT NOT NULL,
                url TEXT,
                enabled INTEGER NOT NULL,
                line_count INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                updated_at TEXT NOT NULL
            );"
        )?;
        self.ensure_defaults()
    }

    pub fn ensure_defaults(&self) -> rusqlite::Result<()> {
        let conn = self.get_conn()?;
        let defaults = vec![
            ("大陆直连主规则 (direct.txt)", "direct.txt", "mainland", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/direct.txt"),
            ("代理过滤主规则 (proxy.txt)", "proxy.txt", "gfw", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/proxy.txt"),
            ("局域私有域名 (private.txt)", "private.txt", "private", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/private.txt"),
            ("中国 IPv4 地址段 (ipv4_china.txt)", "ipv4_china.txt", "china_ip", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv4_china.txt"),
            ("中国 IPv6 地址段 (ipv6_china.txt)", "ipv6_china.txt", "china_ip", "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv6_china.txt"),
        ];

        for (name, file_name, group_type, url) in defaults {
            let exists: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM routing_rule_files WHERE file_name = ?1)",
                params![file_name],
                |r| r.get(0),
            ).unwrap_or(false);

            if !exists {
                let id = Uuid::new_v4().to_string();
                let file_path = self.base_dir.join(file_name);
                let size = fs::metadata(&file_path).map(|m| m.len()).unwrap_or(0);
                let line_count = fs::read_to_string(&file_path)
                    .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
                    .unwrap_or(0);

                let _ = conn.execute(
                    "INSERT OR REPLACE INTO routing_rule_files (id, name, file_name, group_type, url, enabled, line_count, size_bytes, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8)",
                    params![id, name, file_name, group_type, url, line_count as i64, size as i64, chrono::Local::now().to_rfc3339()],
                );
            }
        }
        Ok(())
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
                updated_at: chrono::Local::now().to_rfc3339(),
            });
        }
        res
    }

    pub fn list_rules(&self) -> Vec<RuleFileInfo> {
        let _ = self.ensure_defaults();
        let conn = match self.get_conn() {
            Ok(c) => c,
            Err(_) => return self.get_disk_fallback_rules(),
        };
        let mut stmt = match conn.prepare(
            "SELECT id, name, file_name, group_type, url, enabled, line_count, size_bytes, updated_at FROM routing_rule_files ORDER BY group_type ASC, name ASC"
        ) {
            Ok(s) => s,
            Err(_) => return self.get_disk_fallback_rules(),
        };

        let rows = stmt.query_map([], |r| {
            Ok(RuleFileInfo {
                id: r.get(0)?,
                name: r.get(1)?,
                file_name: r.get(2)?,
                group_type: r.get(3)?,
                url: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                enabled: r.get::<_, i32>(5)? == 1,
                count: r.get::<_, i64>(6)? as usize,
                size_bytes: r.get::<_, i64>(7)? as u64,
                updated_at: r.get(8)?,
            })
        });

        let mut result = Vec::new();
        if let Ok(it) = rows {
            for item in it {
                if let Ok(rf) = item {
                    result.push(rf);
                }
            }
        }
        if result.is_empty() {
            return self.get_disk_fallback_rules();
        }
        result
    }

    pub fn add_rule_file(&self, name: &str, file_name: &str, group_type: &str, url: &str, content: &str) -> anyhow::Result<String> {
        let conn = self.get_conn()?;

        // 智能文件重名处理: 若磁盘或数据库已存在同名文件，自动分配 safe_file_name 避免冲突覆盖
        let mut safe_name = name.to_string();
        let mut safe_file_name = file_name.to_string();
        let p = std::path::Path::new(file_name);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or(file_name);
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("txt");

        let mut idx = 1;
        while self.base_dir.join(&safe_file_name).exists() || {
            let exists_in_db: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM routing_rule_files WHERE file_name = ?1)",
                params![&safe_file_name],
                |r| r.get(0),
            ).unwrap_or(false);
            exists_in_db
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
        conn.execute(
            "INSERT OR REPLACE INTO routing_rule_files (id, name, file_name, group_type, url, enabled, line_count, size_bytes, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7, ?8)",
            params![id, safe_name, safe_file_name, group_type, url, line_count as i64, size as i64, chrono::Local::now().to_rfc3339()],
        )?;

        self.load_local_rules();
        Ok(safe_file_name)
    }

    pub fn toggle_rule_file(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE routing_rule_files SET enabled = (CASE WHEN enabled = 1 THEN 0 ELSE 1 END) WHERE id = ?1",
            params![id],
        )?;
        self.load_local_rules();
        Ok(())
    }

    pub fn edit_rule_info(&self, id: &str, name: &str, group_type: &str, url: &str) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "UPDATE routing_rule_files SET name = ?1, group_type = ?2, url = ?3 WHERE id = ?4",
            params![name, group_type, url, id],
        )?;
        self.load_local_rules();
        Ok(())
    }

    pub fn delete_rule_file(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        let file_name: String = conn.query_row(
            "SELECT file_name FROM routing_rule_files WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )?;
        conn.execute("DELETE FROM routing_rule_files WHERE id = ?1", params![id])?;
        let _ = fs::remove_file(self.base_dir.join(file_name));
        self.load_local_rules();
        Ok(())
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

        if let Ok(conn) = self.get_conn() {
            let _ = conn.execute(
                "UPDATE routing_rule_files SET line_count = ?1, size_bytes = ?2, updated_at = ?3 WHERE file_name = ?4",
                params![line_count as i64, size as i64, chrono::Local::now().to_rfc3339(), file_name],
            );
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
