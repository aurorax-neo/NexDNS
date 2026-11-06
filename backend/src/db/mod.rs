use std::ffi::CString;
use std::path::Path;
use std::sync::Arc;
use rusqlite::{params, params_from_iter, types::Value, Connection};
use tokio::sync::{broadcast, mpsc};
use crate::config::AppConfig;
use crate::pipeline::context::{QueryContext, QueryStatus};
use crate::pipeline::rule_files::RuleFileInfo;

pub mod auth;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DiskStatus {
    pub db_size_bytes: u64,
    pub disk_total_bytes: u64,
    pub disk_avail_bytes: u64,
    pub disk_used_percent: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryLogItem {
    pub id: String,
    pub timestamp: String,
    pub client_ip: String,
    pub client_name: Option<String>,
    pub qname: String,
    pub qtype: String,
    pub status: String,
    pub rcode: String,
    pub matched_rule: Option<String>,
    pub routing_group: Option<String>,
    pub response_ip: Option<String>,
    pub duration_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdblockRuleRecord {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub rule_count: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DnsRewriteRecord {
    pub id: String,
    pub domain: String,
    pub record_type: String,
    pub value: String,
    pub enabled: bool,
}

pub struct Database {
    db_path: String,
    pool: parking_lot::Mutex<Vec<Connection>>,
    log_tx: mpsc::Sender<QueryLogItem>,
    ws_broadcast_tx: broadcast::Sender<QueryLogItem>,
}

pub struct PooledConn<'a> {
    conn: Option<Connection>,
    pool: &'a parking_lot::Mutex<Vec<Connection>>,
}

impl<'a> std::ops::Deref for PooledConn<'a> {
    type Target = Connection;
    fn deref(&self) -> &Self::Target {
        self.conn.as_ref().unwrap()
    }
}

impl<'a> std::ops::DerefMut for PooledConn<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.conn.as_mut().unwrap()
    }
}

impl<'a> Drop for PooledConn<'a> {
    fn drop(&mut self) {
        if let Some(c) = self.conn.take() {
            let mut p = self.pool.lock();
            if p.len() < 16 {
                p.push(c);
            }
        }
    }
}

impl Database {
    pub fn init<P: AsRef<Path>>(path: P) -> anyhow::Result<Arc<Self>> {
        let path_str = path.as_ref().to_str().unwrap().to_string();
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path_str)?;
        let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));

        let auto_vac: i64 = conn.query_row("PRAGMA auto_vacuum", [], |r| r.get(0)).unwrap_or(0);
        if auto_vac != 2 {
            let _ = conn.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;");
        }

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA cache_size = -64000;

             CREATE TABLE IF NOT EXISTS query_logs (
                 id TEXT PRIMARY KEY,
                 timestamp TEXT NOT NULL,
                 client_ip TEXT NOT NULL,
                 client_name TEXT,
                 qname TEXT NOT NULL,
                 qtype TEXT NOT NULL,
                 status TEXT NOT NULL,
                 rcode TEXT NOT NULL,
                 matched_rule TEXT,
                 routing_group TEXT,
                 response_ip TEXT,
                 duration_ms REAL NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_query_logs_time ON query_logs(timestamp DESC);
             CREATE INDEX IF NOT EXISTS idx_query_logs_qname ON query_logs(qname);
             CREATE INDEX IF NOT EXISTS idx_query_logs_status ON query_logs(status);"
        )?;

        let (log_tx, mut log_rx) = mpsc::channel::<QueryLogItem>(10000);
        let (ws_broadcast_tx, _) = broadcast::channel::<QueryLogItem>(1000);

        let db_path_clone = path_str.clone();
        let ws_tx_clone = ws_broadcast_tx.clone();

        // Background async batch writer task with periodic 150ms flush
        tokio::spawn(async move {
            let mut conn = match Connection::open(&db_path_clone) {
                Ok(c) => {
                    let _ = c.busy_timeout(std::time::Duration::from_millis(5000));
                    let _ = c.execute_batch("PRAGMA busy_timeout = 5000;");
                    c
                }
                Err(e) => {
                    tracing::error!("Failed to open DB for background writer: {}", e);
                    return;
                }
            };

            let mut batch = Vec::with_capacity(50);
            let mut ticker = tokio::time::interval(std::time::Duration::from_millis(150));

            loop {
                tokio::select! {
                    Some(item) = log_rx.recv() => {
                        let _ = ws_tx_clone.send(item.clone());
                        batch.push(item);
                        if batch.len() >= 200 {
                            Self::flush_batch(&mut conn, &mut batch);
                        }
                    }
                    _ = ticker.tick() => {
                        if !batch.is_empty() {
                            Self::flush_batch(&mut conn, &mut batch);
                        }
                    }
                    else => {
                        if !batch.is_empty() {
                            Self::flush_batch(&mut conn, &mut batch);
                        }
                        break;
                    }
                }
            }
        });

        Ok(Arc::new(Self {
            db_path: path_str,
            pool: parking_lot::Mutex::new(Vec::new()),
            log_tx,
            ws_broadcast_tx,
        }))
    }

    /// 平滑将旧版 SQLite 中的非日志配置（用户密码、DNS 重写、拦截规则、分流规则）迁移至 config.yaml 与规则目录，
    /// 随后彻底删除所有非日志表，确保数据库仅记录查询日志。
    pub fn migrate_legacy_tables_to_config(&self, cfg: &mut AppConfig, rules_dir: &Path) -> bool {
        let Ok(conn) = self.get_conn() else {
            return false;
        };
        let _ = std::fs::create_dir_all(rules_dir);
        let mut migrated = false;

        let table_exists = |name: &str| -> bool {
            conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                params![name],
                |r| r.get(0),
            )
            .unwrap_or(false)
        };

        // 1. 迁移 users 表中的用户名与密码哈希
        if table_exists("users") {
            if let Ok((username, password_hash)) = conn.query_row(
                "SELECT username, password_hash FROM users LIMIT 1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            ) {
                if !username.trim().is_empty() && !password_hash.trim().is_empty() {
                    // 若当前配置仍是默认 admin1234，则采用数据库里已修改的凭据
                    let (is_default_pass, _) = auth::verify_password("admin1234", &cfg.auth.password, "");
                    if is_default_pass {
                        cfg.auth.username = username;
                        cfg.auth.password = password_hash;
                    }
                }
            }
            migrated = true;
        }

        // 2. 迁移 dns_rewrites 表
        if table_exists("dns_rewrites") {
            if cfg.rewrites.is_empty() {
                if let Ok(mut stmt) = conn.prepare("SELECT id, domain, record_type, value, enabled FROM dns_rewrites ORDER BY domain ASC") {
                    if let Ok(rows) = stmt.query_map([], |r| {
                        Ok(DnsRewriteRecord {
                            id: r.get(0)?,
                            domain: r.get(1)?,
                            record_type: r.get(2)?,
                            value: r.get(3)?,
                            enabled: r.get::<_, i32>(4)? == 1,
                        })
                    }) {
                        for item in rows.flatten() {
                            cfg.rewrites.push(item);
                        }
                    }
                }
            }
            migrated = true;
        }

        // 3. 迁移 adblock_rules 表及规则正文文件
        if table_exists("adblock_rules") {
            if cfg.adblock_rules.is_empty() {
                if let Ok(mut stmt) = conn.prepare("SELECT id, name, url, enabled, rule_count, updated_at, coalesce(content, '') FROM adblock_rules ORDER BY updated_at DESC") {
                    if let Ok(rows) = stmt.query_map([], |r| {
                        Ok((
                            AdblockRuleRecord {
                                id: r.get(0)?,
                                name: r.get(1)?,
                                url: r.get(2)?,
                                enabled: r.get::<_, i32>(3)? == 1,
                                rule_count: r.get::<_, i64>(4)? as usize,
                                updated_at: r.get(5)?,
                            },
                            r.get::<_, String>(6)?,
                        ))
                    }) {
                        for (rec, content) in rows.flatten() {
                            let file_path = rules_dir.join(format!("adblock_{}.txt", rec.id));
                            let _ = std::fs::write(file_path, content);
                            cfg.adblock_rules.push(rec);
                        }
                    }
                }
            }
            migrated = true;
        }

        // 4. 迁移 routing_rule_files 表
        if table_exists("routing_rule_files") {
            if cfg.routing_rules.is_empty() {
                if let Ok(mut stmt) = conn.prepare("SELECT id, name, file_name, group_type, url, enabled, line_count, size_bytes, updated_at FROM routing_rule_files ORDER BY group_type ASC, name ASC") {
                    if let Ok(rows) = stmt.query_map([], |r| {
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
                    }) {
                        for item in rows.flatten() {
                            cfg.routing_rules.push(item);
                        }
                    }
                }
            }
            migrated = true;
        }

        if table_exists("sessions") || table_exists("system_settings") {
            migrated = true;
        }

        if migrated {
            let _ = conn.execute_batch(
                "DROP TABLE IF EXISTS users;
                 DROP TABLE IF EXISTS sessions;
                 DROP TABLE IF EXISTS adblock_rules;
                 DROP TABLE IF EXISTS dns_rewrites;
                 DROP TABLE IF EXISTS routing_rule_files;
                 DROP TABLE IF EXISTS system_settings;"
            );
            drop(conn);
            self.reclaim_space(true);
            tracing::info!("已将旧版数据库中的非日志配置平滑迁移至 config.yaml，并清理非日志表");
        }

        migrated
    }

    fn flush_batch(conn: &mut Connection, batch: &mut Vec<QueryLogItem>) {
        if batch.is_empty() {
            return;
        }
        let tx = match conn.transaction() {
            Ok(t) => t,
            Err(e) => {
                tracing::error!("Failed to begin transaction: {}", e);
                batch.clear();
                return;
            }
        };

        {
            let mut stmt = match tx.prepare(
                "INSERT INTO query_logs (id, timestamp, client_ip, client_name, qname, qtype, status, rcode, matched_rule, routing_group, response_ip, duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
            ) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to prepare stmt: {}", e);
                    batch.clear();
                    return;
                }
            };

            for item in batch.drain(..) {
                let _ = stmt.execute(params![
                    item.id,
                    item.timestamp,
                    item.client_ip,
                    item.client_name,
                    item.qname,
                    item.qtype,
                    item.status,
                    item.rcode,
                    item.matched_rule,
                    item.routing_group,
                    item.response_ip,
                    item.duration_ms,
                ]);
            }
        }

        let _ = tx.commit();
    }

    pub fn record_query(&self, ctx: &QueryContext) {
        let status_str = match ctx.status {
            QueryStatus::Allowed => "allowed",
            QueryStatus::Blocked => "blocked",
            QueryStatus::Cached => "cached",
            QueryStatus::Rewritten => "rewritten",
            QueryStatus::Error => "error",
        };

        let resp_ip = ctx.fastest_ip.map(|ip| ip.to_string()).or_else(|| {
            if let Some(ref resp) = ctx.response {
                // 优先提取最终解析出的 A / AAAA 真实 IP（DNS 应答中 CNAME 位于首部，避免被首条 CNAME 提前截断而丢失真实 IP）
                for ans in &resp.answers {
                    if let hickory_proto::rr::RData::A(a) = &ans.data {
                        return Some(a.0.to_string());
                    } else if let hickory_proto::rr::RData::AAAA(aaaa) = &ans.data {
                        return Some(aaaa.0.to_string());
                    }
                }
                for ans in &resp.answers {
                    if let hickory_proto::rr::RData::CNAME(cname) = &ans.data {
                        return Some(format!("CNAME: {}", cname.to_utf8().trim_end_matches('.')));
                    } else if let hickory_proto::rr::RData::PTR(ptr) = &ans.data {
                        return Some(format!("PTR: {}", ptr.to_utf8().trim_end_matches('.')));
                    }
                }
            }
            None
        });

        let item = QueryLogItem {
            id: ctx.id.to_string(),
            timestamp: ctx.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
            client_ip: ctx.client_ip.to_string(),
            client_name: ctx.client_name.clone(),
            qname: ctx.qname.clone(),
            qtype: ctx.qtype.to_string(),
            status: status_str.to_string(),
            rcode: ctx.rcode.to_string(),
            matched_rule: ctx.matched_rule.clone(),
            routing_group: ctx.routing_group.clone(),
            response_ip: resp_ip,
            duration_ms: ctx.duration_ms,
        };

        let _ = self.log_tx.try_send(item);
    }

    pub fn subscribe_ws(&self) -> broadcast::Receiver<QueryLogItem> {
        self.ws_broadcast_tx.subscribe()
    }

    pub fn get_conn(&self) -> rusqlite::Result<PooledConn<'_>> {
        let maybe_conn = self.pool.lock().pop();
        let conn = match maybe_conn {
            Some(c) => c,
            None => {
                let c = Connection::open(&self.db_path)?;
                let _ = c.busy_timeout(std::time::Duration::from_millis(5000));
                let _ = c.execute_batch("PRAGMA busy_timeout = 5000;");
                c
            }
        };
        Ok(PooledConn {
            conn: Some(conn),
            pool: &self.pool,
        })
    }

    pub fn query_logs(
        &self,
        limit: usize,
        offset: usize,
        search: Option<&str>,
        status: Option<&str>,
        client: Option<&str>,
    ) -> rusqlite::Result<(Vec<QueryLogItem>, usize)> {
        let conn = self.get_conn()?;
        let mut count_sql = "SELECT COUNT(*) FROM query_logs WHERE 1=1".to_string();
        let mut sql = "SELECT id, timestamp, client_ip, client_name, qname, qtype, status, rcode, matched_rule, routing_group, response_ip, duration_ms FROM query_logs WHERE 1=1".to_string();

        let mut count_params = Vec::<Value>::new();

        if let Some(s) = search {
            let s_trim = s.trim();
            if !s_trim.is_empty() {
                count_sql.push_str(" AND qname LIKE ?");
                sql.push_str(" AND qname LIKE ?");
                count_params.push(Value::Text(format!("%{}%", s_trim)));
            }
        }
        if let Some(st) = status {
            let st_trim = st.trim();
            if !st_trim.is_empty() && st_trim != "all" {
                count_sql.push_str(" AND status = ?");
                sql.push_str(" AND status = ?");
                count_params.push(Value::Text(st_trim.to_string()));
            }
        }
        if let Some(c) = client {
            let c_trim = c.trim();
            if !c_trim.is_empty() {
                count_sql.push_str(" AND client_ip = ?");
                sql.push_str(" AND client_ip = ?");
                count_params.push(Value::Text(c_trim.to_string()));
            }
        }

        let total: usize = conn.query_row(&count_sql, params_from_iter(count_params.iter()), |r| r.get(0))?;

        sql.push_str(" ORDER BY rowid DESC LIMIT ? OFFSET ?");
        let mut query_params = count_params;
        query_params.push(Value::Integer(limit as i64));
        query_params.push(Value::Integer(offset as i64));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(query_params.iter()), |row| {
            Ok(QueryLogItem {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                client_ip: row.get(2)?,
                client_name: row.get(3)?,
                qname: row.get(4)?,
                qtype: row.get(5)?,
                status: row.get(6)?,
                rcode: row.get(7)?,
                matched_rule: row.get(8)?,
                routing_group: row.get(9)?,
                response_ip: row.get(10)?,
                duration_ms: row.get(11)?,
            })
        })?;

        let mut items = Vec::new();
        for r in rows {
            items.push(r?);
        }

        Ok((items, total))
    }

    pub fn clear_logs(&self) -> rusqlite::Result<()> {
        {
            let conn = self.get_conn()?;
            conn.execute("DELETE FROM query_logs", [])?;
        }
        self.reclaim_space(true);
        Ok(())
    }

    pub fn reclaim_space(&self, full_vacuum: bool) {
        self.pool.lock().clear();
        if let Ok(conn) = Connection::open(&self.db_path) {
            let _ = conn.busy_timeout(std::time::Duration::from_millis(5000));
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
            if full_vacuum {
                let _ = conn.execute_batch("VACUUM;");
            } else {
                let _ = conn.execute_batch("PRAGMA incremental_vacuum(2000);");
            }
            let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
        }
    }

    pub fn cleanup_logs_older_than(&self, mins: u32) -> rusqlite::Result<usize> {
        let mins = mins.max(1);
        let cutoff_dt = chrono::Local::now() - chrono::Duration::minutes(mins as i64);
        let cutoff_str = cutoff_dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, false);
        let cutoff_epoch = cutoff_dt.timestamp();

        let deleted = {
            let conn = self.get_conn()?;
            conn.execute(
                "DELETE FROM query_logs
                 WHERE timestamp < ?1
                    OR COALESCE(unixepoch(timestamp), 0) < ?2",
                rusqlite::params![cutoff_str, cutoff_epoch],
            )?
        };

        if deleted > 0 {
            self.reclaim_space(deleted >= 5000);
        }
        Ok(deleted)
    }

    pub fn get_disk_status(&self) -> DiskStatus {
        let mut db_size_bytes = 0u64;
        for suffix in ["", "-wal", "-shm"] {
            let p = format!("{}{}", self.db_path, suffix);
            if let Ok(meta) = std::fs::metadata(&p) {
                db_size_bytes = db_size_bytes.saturating_add(meta.len());
            }
        }

        let check_dir = Path::new(&self.db_path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));

        let (disk_total_bytes, disk_avail_bytes, disk_used_percent) = query_filesystem_space(check_dir);

        DiskStatus {
            db_size_bytes,
            disk_total_bytes,
            disk_avail_bytes,
            disk_used_percent,
        }
    }

    /// 磁盘与日志数据库容量巡检：当磁盘使用率超阈值、剩余空间不足或日志库体积超上限时，
    /// 自动分批淘汰最旧的查询日志并执行 TRUNCATE + VACUUM 释放磁盘空间，防止日志撑爆机器。
    pub fn enforce_disk_and_db_limits(
        &self,
        max_disk_usage_percent: u8,
        min_free_disk_mb: u64,
        max_db_size_mb: u64,
    ) -> rusqlite::Result<usize> {
        let max_pct = max_disk_usage_percent.clamp(50, 98) as f64;
        let min_free_bytes = min_free_disk_mb.max(64).saturating_mul(1024 * 1024);
        let max_db_bytes = max_db_size_mb.max(32).saturating_mul(1024 * 1024);

        let mut total_deleted = 0usize;

        for _pass in 0..4 {
            let status = self.get_disk_status();
            let low_free_space = status.disk_total_bytes > 0 && status.disk_avail_bytes <= min_free_bytes;
            let high_disk_pct = status.disk_total_bytes > 0
                && status.disk_used_percent >= max_pct
                && status.db_size_bytes >= 8 * 1024 * 1024;
            let disk_pressure = low_free_space || high_disk_pct;
            let db_pressure = status.db_size_bytes >= max_db_bytes;

            if !disk_pressure && !db_pressure {
                break;
            }

            // 若磁盘已极度告警（>=95% 且日志库 >=8MB，或剩余空间不足 128MB），直接清空全部日志保命
            let critical_disk = status.disk_total_bytes > 0
                && ((status.disk_used_percent >= 95.0 && status.db_size_bytes >= 8 * 1024 * 1024)
                    || status.disk_avail_bytes < 128 * 1024 * 1024);

            let deleted_this_pass = {
                let conn = self.get_conn()?;
                let row_count: usize = conn.query_row("SELECT COUNT(*) FROM query_logs", [], |r| r.get(0)).unwrap_or(0);
                if row_count == 0 {
                    0
                } else if critical_disk {
                    conn.execute("DELETE FROM query_logs", [])?
                } else {
                    let batch_to_delete = (row_count / 3).max(5000);
                    conn.execute(
                        "DELETE FROM query_logs WHERE rowid IN (
                            SELECT rowid FROM query_logs ORDER BY rowid ASC LIMIT ?1
                        )",
                        params![batch_to_delete as i64],
                    )?
                }
            };

            total_deleted += deleted_this_pass;
            self.reclaim_space(true);

            if deleted_this_pass == 0 {
                break;
            }
        }

        Ok(total_deleted)
    }
}

#[cfg(unix)]
fn query_filesystem_space(path: &Path) -> (u64, u64, f64) {
    let path_str = path.to_string_lossy();
    let Ok(c_path) = CString::new(path_str.as_bytes()) else {
        return (0, 0, 0.0);
    };
    unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) == 0 {
            let block_size = if stat.f_frsize > 0 {
                stat.f_frsize as u64
            } else {
                stat.f_bsize as u64
            };
            let total = (stat.f_blocks as u64).saturating_mul(block_size);
            let avail = (stat.f_bavail as u64).saturating_mul(block_size);
            let used = total.saturating_sub(avail);
            let pct = if total > 0 {
                ((used as f64 / total as f64) * 1000.0).round() / 10.0
            } else {
                0.0
            };
            return (total, avail, pct);
        }
    }
    (0, 0, 0.0)
}

#[cfg(not(unix))]
fn query_filesystem_space(_path: &Path) -> (u64, u64, f64) {
    (0, 0, 0.0)
}
