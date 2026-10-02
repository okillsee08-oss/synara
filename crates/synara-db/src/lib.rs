use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;
use synara_events::Event;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let c = Connection::open(path)?;
        let db = Self { conn: c };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_memory() -> Result<Self> {
        let db = Self {
            conn: Connection::open_in_memory()?,
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS schema_migrations(
                 version INTEGER PRIMARY KEY,
                 applied_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS events(
                 event_id TEXT PRIMARY KEY,
                 sequence INTEGER NOT NULL UNIQUE,
                 timestamp TEXT NOT NULL,
                 scope TEXT NOT NULL,
                 entity_id TEXT NOT NULL,
                 event_type TEXT NOT NULL,
                 version INTEGER NOT NULL,
                 payload TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_events_sequence ON events(sequence);
             CREATE INDEX IF NOT EXISTS idx_events_scope_entity ON events(scope,entity_id);
             CREATE TABLE IF NOT EXISTS projections(
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL,
                 updated_sequence INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE IF NOT EXISTS projects(
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 root_path TEXT NOT NULL,
                 created_sequence INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS workspaces(
                 id TEXT PRIMARY KEY,
                 project_id TEXT NOT NULL,
                 root_path TEXT NOT NULL,
                 created_sequence INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS turns(
                 id TEXT PRIMARY KEY,
                 thread_id TEXT NOT NULL,
                 status TEXT NOT NULL,
                 created_sequence INTEGER NOT NULL,
                 updated_sequence INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS approvals(
                 id TEXT PRIMARY KEY,
                 tool_call_id TEXT NOT NULL,
                 approved INTEGER,
                 updated_sequence INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS threads(
                 id TEXT PRIMARY KEY,
                 workspace_id TEXT NOT NULL,
                 title TEXT,
                 created_sequence INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS messages(
                 id TEXT PRIMARY KEY,
                 thread_id TEXT NOT NULL,
                 role TEXT NOT NULL,
                 content TEXT NOT NULL,
                 created_sequence INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS provider_sessions(
                 id TEXT PRIMARY KEY,
                 provider_kind TEXT NOT NULL,
                 thread_id TEXT,
                 remote_thread_id TEXT,
                 status TEXT NOT NULL,
                 created_sequence INTEGER NOT NULL,
                 updated_sequence INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_provider_sessions_status ON provider_sessions(status);
             CREATE INDEX IF NOT EXISTS idx_provider_sessions_provider ON provider_sessions(provider_kind);
             CREATE INDEX IF NOT EXISTS idx_provider_sessions_thread ON provider_sessions(thread_id);",
        )?;

        let has_provider_thread_id: u64 = self.conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('provider_sessions') WHERE name='thread_id'",
            [],
            |r| r.get(0),
        )?;
        if has_provider_thread_id == 0 {
            self.conn.execute(
                "ALTER TABLE provider_sessions ADD COLUMN thread_id TEXT",
                [],
            )?;
        }

        Ok(())
    }

    pub fn append_event(&self, e: &Event) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        let latest: u64 =
            tx.query_row("SELECT COALESCE(MAX(sequence),0) FROM events", [], |r| {
                r.get(0)
            })?;
        if e.sequence != latest + 1 {
            anyhow::bail!(
                "event sequence fence violated: expected {}, got {}",
                latest + 1,
                e.sequence
            );
        }

        tx.execute(
            "INSERT INTO events(event_id,sequence,timestamp,scope,entity_id,event_type,version,payload)
             VALUES(?,?,?,?,?,?,?,?)",
            params![
                e.event_id.to_string(), e.sequence, e.timestamp.to_rfc3339(),
                e.scope, e.entity_id.to_string(), e.event_type, e.version, e.payload.to_string()
            ],
        )?;

        match e.event_type.as_str() {
            "ProjectCreated" => {
                tx.execute(
                    "INSERT INTO projects(id,name,root_path,created_sequence) VALUES(?,?,?,?)",
                    params![
                        e.entity_id.to_string(),
                        e.payload["name"].as_str().unwrap_or_default(),
                        e.payload["root_path"].as_str().unwrap_or_default(),
                        e.sequence
                    ],
                )?;
            }
            "WorkspaceCreated" => {
                tx.execute(
                    "INSERT INTO workspaces(id,project_id,root_path,created_sequence) VALUES(?,?,?,?)",
                    params![
                        e.entity_id.to_string(),
                        e.payload["project_id"].to_string().trim_matches('"'),
                        e.payload["root_path"].as_str().unwrap_or_default(),
                        e.sequence
                    ],
                )?;
            }
            "ThreadCreated" => {
                tx.execute(
                    "INSERT INTO threads(id,workspace_id,title,created_sequence) VALUES(?,?,?,?)",
                    params![
                        e.entity_id.to_string(),
                        e.payload["workspace_id"].to_string().trim_matches('"'),
                        e.payload["title"].as_str(),
                        e.sequence
                    ],
                )?;
            }
            "TurnStarted" => {
                tx.execute(
                    "INSERT INTO turns(id,thread_id,status,created_sequence,updated_sequence) VALUES(?,?,?,?,?)",
                    params![
                        e.entity_id.to_string(),
                        e.payload["thread_id"].to_string().trim_matches('"'),
                        "running",
                        e.sequence,
                        e.sequence
                    ],
                )?;
            }
            "TurnStopped" => {
                tx.execute(
                    "UPDATE turns SET status='cancelled', updated_sequence=? WHERE id=?",
                    params![e.sequence, e.entity_id.to_string()],
                )?;
            }
            "TurnFailed" => {
                tx.execute(
                    "UPDATE turns SET status='failed', updated_sequence=? WHERE id=?",
                    params![e.sequence, e.entity_id.to_string()],
                )?;
            }
            "ToolApproved" | "ToolRejected" => {
                tx.execute(
                    "INSERT INTO approvals(id,tool_call_id,approved,updated_sequence) VALUES(?,?,?,?)
                     ON CONFLICT(id) DO UPDATE SET approved=excluded.approved,updated_sequence=excluded.updated_sequence",
                    params![
                        e.entity_id.to_string(),
                        e.payload["tool_call_id"].to_string().trim_matches('"'),
                        e.payload["approved"].as_bool().map(|v| if v { 1 } else { 0 }),
                        e.sequence
                    ],
                )?;
            }
            "ProviderSessionStarted" => {
                tx.execute(
                    "INSERT INTO provider_sessions(id,provider_kind,thread_id,remote_thread_id,status,created_sequence,updated_sequence)
                     VALUES(?,?,?,?,?,?,?)",
                    params![
                        e.entity_id.to_string(),
                        e.payload["provider_kind"].as_str().unwrap_or_default(),
                        e.payload["thread"].as_str(),
                        e.payload["remote_thread_id"].as_str(),
                        "running",
                        e.sequence,
                        e.sequence
                    ],
                )?;
            }
            "ProviderCompleted" => {
                tx.execute(
                    "UPDATE provider_sessions SET status='completed', updated_sequence=? WHERE id=?",
                    params![e.sequence, e.entity_id.to_string()],
                )?;
                tx.execute(
                    "UPDATE turns SET status='completed', updated_sequence=?
                     WHERE thread_id=(SELECT thread_id FROM provider_sessions WHERE id=?)
                       AND status='running'",
                    params![e.sequence, e.entity_id.to_string()],
                )?;
            }
            "ProviderFailed" => {
                tx.execute(
                    "UPDATE provider_sessions SET status='failed', updated_sequence=? WHERE id=?",
                    params![e.sequence, e.entity_id.to_string()],
                )?;
                tx.execute(
                    "UPDATE turns SET status='failed', updated_sequence=?
                     WHERE thread_id=(SELECT thread_id FROM provider_sessions WHERE id=?)
                       AND status='running'",
                    params![e.sequence, e.entity_id.to_string()],
                )?;
            }
            "MessageCreated" => {
                tx.execute(
                    "INSERT INTO messages(id,thread_id,role,content,created_sequence) VALUES(?,?,?,?,?)",
                    params![
                        e.entity_id.to_string(),
                        e.payload["thread_id"].to_string().trim_matches('"'),
                        "user",
                        e.payload["content"].as_str().unwrap_or_default(),
                        e.sequence
                    ],
                )?;
            }
            _ => {}
        }

        tx.commit()?;
        Ok(())
    }

    pub fn events_after(&self, seq: u64) -> Result<Vec<Event>> {
        let mut stmt = self.conn.prepare(
            "SELECT event_id,sequence,timestamp,scope,entity_id,event_type,version,payload
             FROM events WHERE sequence>? ORDER BY sequence",
        )?;
        let rows = stmt.query_map([seq], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, u64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, u32>(6)?,
                r.get::<_, String>(7)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let r = row?;
            out.push(Event {
                event_id: r.0.parse()?,
                sequence: r.1,
                timestamp: chrono::DateTime::parse_from_rfc3339(&r.2)?.with_timezone(&chrono::Utc),
                scope: r.3,
                entity_id: r.4.parse()?,
                event_type: r.5,
                version: r.6,
                payload: serde_json::from_str(&r.7)?,
            });
        }
        Ok(out)
    }

    pub fn project_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM projects", [], |r| r.get(0))?)
    }

    pub fn thread_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM threads", [], |r| r.get(0))?)
    }

    pub fn workspace_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM workspaces", [], |r| r.get(0))?)
    }

    pub fn turn_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM turns", [], |r| r.get(0))?)
    }

    pub fn approval_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM approvals", [], |r| r.get(0))?)
    }

    pub fn message_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?)
    }

    pub fn provider_session_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM provider_sessions", [], |r| r.get(0))?)
    }

    pub fn project_exists(&self, id: synara_core::EntityId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?)",
            [id.to_string()],
            |r| r.get(0),
        )?)
    }

    pub fn workspace_exists(&self, id: synara_core::EntityId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id=?)",
            [id.to_string()],
            |r| r.get(0),
        )?)
    }

    pub fn thread_exists(&self, id: synara_core::EntityId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM threads WHERE id=?)",
            [id.to_string()],
            |r| r.get(0),
        )?)
    }

    pub fn turn_exists(&self, id: synara_core::EntityId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM turns WHERE id=?)",
            [id.to_string()],
            |r| r.get(0),
        )?)
    }

    pub fn running_turn_exists(&self, thread_id: synara_core::EntityId) -> Result<bool> {
        Ok(self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM turns WHERE thread_id=? AND status='running')",
            [thread_id.to_string()],
            |r| r.get(0),
        )?)
    }

    pub fn list_projects(&self) -> Result<Vec<(synara_core::EntityId, String, String, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,name,root_path,created_sequence FROM projects ORDER BY created_sequence",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, name, root_path, sequence) = row?;
            out.push((id.parse()?, name, root_path, sequence));
        }
        Ok(out)
    }

    pub fn list_workspaces(
        &self,
    ) -> Result<Vec<(synara_core::EntityId, synara_core::EntityId, String, u64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id,project_id,root_path,created_sequence FROM workspaces ORDER BY created_sequence",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, project_id, root_path, sequence) = row?;
            out.push((id.parse()?, project_id.parse()?, root_path, sequence));
        }
        Ok(out)
    }

    pub fn list_threads(
        &self,
    ) -> Result<
        Vec<(
            synara_core::EntityId,
            synara_core::EntityId,
            Option<String>,
            u64,
        )>,
    > {
        let mut stmt = self.conn.prepare(
            "SELECT id,workspace_id,title,created_sequence FROM threads ORDER BY created_sequence",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, u64>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, workspace_id, title, sequence) = row?;
            out.push((id.parse()?, workspace_id.parse()?, title, sequence));
        }
        Ok(out)
    }

    pub fn list_messages(
        &self,
    ) -> Result<
        Vec<(
            synara_core::EntityId,
            synara_core::EntityId,
            String,
            String,
            u64,
        )>,
    > {
        let mut stmt = self.conn.prepare(
            "SELECT id,thread_id,role,content,created_sequence FROM messages ORDER BY created_sequence",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, u64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, thread_id, role, content, sequence) = row?;
            out.push((id.parse()?, thread_id.parse()?, role, content, sequence));
        }
        Ok(out)
    }

    pub fn projection(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM projections WHERE key=?")?;
        let mut rows = stmt.query([key])?;
        Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
    }

    pub fn set_projection(&self, key: &str, value: &str, sequence: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO projections(key,value,updated_sequence) VALUES(?,?,?)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value,updated_sequence=excluded.updated_sequence",
            params![key, value, sequence],
        )?;
        Ok(())
    }

    pub fn latest_sequence(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COALESCE(MAX(sequence),0) FROM events", [], |r| {
                r.get(0)
            })?)
    }
}
