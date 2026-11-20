//! SQLite event log for `scenes why`, `stats` and `suggest`.
//! All writes go through the EventSink trait, so tests use an in-memory mock.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OpenFlags, params};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LogError {
    #[error("database: {0}")]
    Db(String),
    #[error("log is busy")]
    Busy,
}

#[derive(Debug, Clone)]
pub struct EventRecord {
    pub kind: String,
    pub scene: String,
    pub signals: String,
    pub detail: String,
    pub ts: i64,
}

pub trait EventSink: Send + Sync {
    fn record(&self, record: &EventRecord) -> Result<(), LogError>;
}

pub struct SqliteLog {
    conn: Mutex<Connection>,
}

impl SqliteLog {
    pub fn open(path: &Path) -> Result<Self, LogError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| LogError::Db(e.to_string()))?;
        }
        let conn = Connection::open(path).map_err(|e| LogError::Db(e.to_string()))?;
        Self::setup(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_readonly(path: &Path) -> Result<Self, LogError> {
        if !path.exists() {
            return Err(LogError::Db("no event log yet".into()));
        }
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|e| LogError::Db(e.to_string()))?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn setup(conn: &Connection) -> Result<(), LogError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ts INTEGER NOT NULL,
                kind TEXT NOT NULL,
                scene TEXT NOT NULL,
                signals TEXT NOT NULL,
                detail TEXT NOT NULL
             );",
        )
        .map_err(|e| LogError::Db(e.to_string()))
    }

    pub fn last_decision(&self) -> Result<Option<(i64, String, String, String)>, LogError> {
        let conn = self.conn.lock().map_err(|_| LogError::Busy)?;
        let mut stmt = conn
            .prepare("SELECT ts, scene, signals, detail FROM events WHERE kind='decision' ORDER BY ts DESC, id DESC LIMIT 1")
            .map_err(|e| LogError::Db(e.to_string()))?;
        let mut rows = stmt.query([]).map_err(|e| LogError::Db(e.to_string()))?;
        match rows.next().map_err(|e| LogError::Db(e.to_string()))? {
            Some(row) => Ok(Some((
                row.get(0).map_err(|e| LogError::Db(e.to_string()))?,
                row.get(1).map_err(|e| LogError::Db(e.to_string()))?,
                row.get(2).map_err(|e| LogError::Db(e.to_string()))?,
                row.get(3).map_err(|e| LogError::Db(e.to_string()))?,
            ))),
            None => Ok(None),
        }
    }

    pub fn stats(&self) -> Result<Vec<(String, i64)>, LogError> {
        let conn = self.conn.lock().map_err(|_| LogError::Busy)?;
        let mut stmt = conn
            .prepare(
                "SELECT scene, SUM(CAST(detail AS INTEGER)) AS seconds
                 FROM events WHERE kind='stay' GROUP BY scene ORDER BY seconds DESC",
            )
            .map_err(|e| LogError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1).unwrap_or_default(),
                ))
            })
            .map_err(|e| LogError::Db(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|e| LogError::Db(e.to_string()))?);
        }
        Ok(out)
    }

    /// Draft scenes from observed decisions. Never applied automatically.
    pub fn suggest(&self) -> Result<String, LogError> {
        let conn = self.conn.lock().map_err(|_| LogError::Busy)?;
        let mut stmt = conn
            .prepare("SELECT scene, signals FROM events WHERE kind='decision' AND scene != ''")
            .map_err(|e| LogError::Db(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1).unwrap_or_default(),
                ))
            })
            .map_err(|e| LogError::Db(e.to_string()))?;
        let mut counts: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, usize>,
        > = std::collections::BTreeMap::new();
        for row in rows {
            let (scene, signals) = row.map_err(|e| LogError::Db(e.to_string()))?;
            let value: serde_json::Value =
                serde_json::from_str(&signals).unwrap_or(serde_json::Value::Null);
            if let Some(class) = value.get("window_class").and_then(|v| v.as_str())
                && !class.is_empty()
            {
                *counts
                    .entry(scene)
                    .or_default()
                    .entry(class.to_string())
                    .or_insert(0) += 1;
            }
        }
        if counts.is_empty() {
            return Ok("# No decisions logged yet — nothing to suggest.\n".into());
        }
        let mut text = String::from(
            "# DRAFT scenes suggested from your event log.\n# Review, edit, then copy into scenes.toml. Nothing is auto-applied.\n\n",
        );
        for (scene, classes) in counts {
            let mut pairs: Vec<_> = classes.into_iter().collect();
            pairs.sort_by_key(|a| std::cmp::Reverse(a.1));
            let top: Vec<String> = pairs
                .into_iter()
                .take(4)
                .map(|(class, _)| format!("\"{}\"", class))
                .collect();
            text.push_str(&format!("[scene.{scene}]\npriority = 50\n\n  [[scene.{scene}.when]]\n  window_class = [{}]\n\n  [[scene.{scene}.do]]\nkind = \"omarchy\"\nrun = \"theme set 'Tokyo Night'\"\n\n", top.join(", ")));
        }
        Ok(text)
    }
}

impl EventSink for SqliteLog {
    fn record(&self, record: &EventRecord) -> Result<(), LogError> {
        let conn = self.conn.lock().map_err(|_| LogError::Busy)?;
        conn.execute(
            "INSERT INTO events (ts, kind, scene, signals, detail) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.ts,
                record.kind,
                record.scene,
                record.signals,
                record.detail
            ],
        )
        .map_err(|e| LogError::Db(e.to_string()))?;
        Ok(())
    }
}

#[derive(Default)]
pub struct MockSink {
    pub records: Mutex<Vec<EventRecord>>,
}

impl EventSink for MockSink {
    fn record(&self, record: &EventRecord) -> Result<(), LogError> {
        self.records
            .lock()
            .map_err(|_| LogError::Busy)?
            .push(record.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(kind: &str, scene: &str, signals: &str, detail: &str) -> EventRecord {
        EventRecord {
            kind: kind.into(),
            scene: scene.into(),
            signals: signals.into(),
            detail: detail.into(),
            ts: 1_700_000_000,
        }
    }

    #[test]
    fn logs_and_queries() {
        let log = SqliteLog::open(Path::new(":memory:")).unwrap_or_else(|e| panic!("{e}"));
        log.record(&record(
            "decision",
            "focus",
            r#"{"window_class":"code"}"#,
            "matched",
        ))
        .unwrap_or_else(|e| panic!("{e}"));
        log.record(&record("stay", "focus", "", "42"))
            .unwrap_or_else(|e| panic!("{e}"));
        let last = log
            .last_decision()
            .unwrap_or_else(|e| panic!("{e}"))
            .unwrap_or_default();
        assert_eq!(last.1, "focus");
        assert_eq!(log.stats().unwrap_or_default()[0], ("focus".into(), 42));
        assert!(log.suggest().unwrap_or_default().contains("window_class"));
    }
}
