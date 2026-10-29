use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use rusqlite::{params, Connection};
use tokio::sync::{broadcast, mpsc};
use crate::pipeline::context::{QueryContext, QueryStatus};

pub mod auth;

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
    log_tx: mpsc::Sender<QueryLogItem>,
    ws_broadcast_tx: broadcast::Sender<QueryLogItem>,
}

impl Database {
    pub fn init<P: AsRef<Path>>(path: P) -> anyhow::Result<Arc<Self>> {
        let path_str = path.as_ref().to_str().unwrap().to_string();
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path_str)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
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
             CREATE INDEX IF NOT EXISTS idx_query_logs_status ON query_logs(status);

             CREATE TABLE IF NOT EXISTS adblock_rules (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 url TEXT NOT NULL,
                 enabled INTEGER NOT NULL,
                 rule_count INTEGER NOT NULL,
                 updated_at TEXT NOT NULL,
                 content TEXT
             );

             CREATE TABLE IF NOT EXISTS dns_rewrites (
                 id TEXT PRIMARY KEY,
                 domain TEXT NOT NULL,
                 record_type TEXT NOT NULL,
                 value TEXT NOT NULL,
                 enabled INTEGER NOT NULL
             );

             CREATE TABLE IF NOT EXISTS system_settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS routing_rule_files (
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

        auth::AuthManager::init_tables(&conn)?;

        let (log_tx, mut log_rx) = mpsc::channel::<QueryLogItem>(10000);
        let (ws_broadcast_tx, _) = broadcast::channel::<QueryLogItem>(1000);

        let db_path_clone = path_str.clone();
        let ws_tx_clone = ws_broadcast_tx.clone();

        // Background async batch writer task with periodic 150ms flush
        tokio::spawn(async move {
            let mut conn = match Connection::open(&db_path_clone) {
                Ok(c) => c,
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
                        if batch.len() >= 10 {
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
            log_tx,
            ws_broadcast_tx,
        }))
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
            let mut stmt = match tx.prepare_cached(
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
                for ans in &resp.answers {
                    if let hickory_proto::rr::RData::A(a) = &ans.data {
                        return Some(a.0.to_string());
                    } else if let hickory_proto::rr::RData::AAAA(aaaa) = &ans.data {
                        return Some(aaaa.0.to_string());
                    } else if let hickory_proto::rr::RData::CNAME(cname) = &ans.data {
                        return Some(format!("CNAME: {}", cname.to_utf8().trim_end_matches('.')));
                    }
                }
            }
            None
        });

        let item = QueryLogItem {
            id: ctx.id.to_string(),
            timestamp: ctx.timestamp.to_rfc3339(),
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

    pub fn get_conn(&self) -> rusqlite::Result<Connection> {
        Connection::open(&self.db_path)
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
        let mut sql = "SELECT id, timestamp, client_ip, client_name, qname, qtype, status, rcode, matched_rule, routing_group, response_ip, duration_ms FROM query_logs WHERE 1=1".to_string();
        let mut count_sql = "SELECT COUNT(*) FROM query_logs WHERE 1=1".to_string();

        let mut where_clause = String::new();
        if let Some(s) = search {
            if !s.is_empty() {
                where_clause.push_str(&format!(" AND qname LIKE '%{}%'", s.replace('\'', "''")));
            }
        }
        if let Some(st) = status {
            if !st.is_empty() && st != "all" {
                where_clause.push_str(&format!(" AND status = '{}'", st.replace('\'', "''")));
            }
        }
        if let Some(c) = client {
            if !c.is_empty() {
                where_clause.push_str(&format!(" AND client_ip = '{}'", c.replace('\'', "''")));
            }
        }

        sql.push_str(&where_clause);
        count_sql.push_str(&where_clause);

        let total: usize = conn.query_row(&count_sql, [], |r| r.get(0))?;

        sql.push_str(&format!(" ORDER BY timestamp DESC LIMIT {} OFFSET {}", limit, offset));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| {
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
        let conn = self.get_conn()?;
        conn.execute("DELETE FROM query_logs", [])?;
        conn.execute("VACUUM", [])?;
        Ok(())
    }

    pub fn get_rules(&self) -> rusqlite::Result<Vec<AdblockRuleRecord>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("SELECT id, name, url, enabled, rule_count, updated_at FROM adblock_rules ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(AdblockRuleRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                url: r.get(2)?,
                enabled: r.get::<_, i32>(3)? == 1,
                rule_count: r.get::<_, i64>(4)? as usize,
                updated_at: r.get(5)?,
            })
        })?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn save_rule(&self, rule: &AdblockRuleRecord, content: &str) -> rusqlite::Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO adblock_rules (id, name, url, enabled, rule_count, updated_at, content)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rule.id,
                rule.name,
                rule.url,
                if rule.enabled { 1 } else { 0 },
                rule.rule_count as i64,
                rule.updated_at,
                content,
            ],
        )?;
        Ok(())
    }

    pub fn delete_rule(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.get_conn()?;
        conn.execute("DELETE FROM adblock_rules WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn get_rewrites(&self) -> rusqlite::Result<Vec<DnsRewriteRecord>> {
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare("SELECT id, domain, record_type, value, enabled FROM dns_rewrites ORDER BY domain ASC")?;
        let rows = stmt.query_map([], |r| {
            Ok(DnsRewriteRecord {
                id: r.get(0)?,
                domain: r.get(1)?,
                record_type: r.get(2)?,
                value: r.get(3)?,
                enabled: r.get::<_, i32>(4)? == 1,
            })
        })?;
        let mut res = Vec::new();
        for r in rows {
            res.push(r?);
        }
        Ok(res)
    }

    pub fn save_rewrite(&self, rec: &DnsRewriteRecord) -> rusqlite::Result<()> {
        let conn = self.get_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO dns_rewrites (id, domain, record_type, value, enabled)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![rec.id, rec.domain, rec.record_type, rec.value, if rec.enabled { 1 } else { 0 }],
        )?;
        Ok(())
    }

    pub fn delete_rewrite(&self, id: &str) -> rusqlite::Result<()> {
        let conn = self.get_conn()?;
        conn.execute("DELETE FROM dns_rewrites WHERE id = ?1", params![id])?;
        Ok(())
    }
}
