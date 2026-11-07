use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Definition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<AiBlock>,
    #[serde(default)]
    pub connections: Vec<GraphConnection>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AiBlock {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub position: Position,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GraphConnection {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedDefinition {
    pub definition: Definition,
    pub revision: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveDefinition {
    pub definition: Definition,
    pub expected_revision: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "code", rename_all = "camelCase")]
pub enum Error {
    Validation { message: String },
    Conflict { message: String },
    NotFound { message: String },
    Storage { message: String },
    UnsupportedSchema { message: String },
    Busy { message: String },
}

fn storage(_: impl std::fmt::Display) -> Error {
    Error::Storage { message: "Desktop storage unavailable. Check free space and access, then retry; your edits are retained.".into() }
}

fn private_path(path: &Path, directory: bool) -> Result<(), Error> {
    let metadata = path.symlink_metadata().map_err(storage)?;
    if metadata.file_type().is_symlink()
        || metadata.is_dir() != directory
        || (!directory && !metadata.is_file())
    {
        return Err(storage("unsafe storage path"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            path,
            std::fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
        )
        .map_err(storage)?;
    }
    Ok(())
}

fn validation(message: &str) -> Error {
    Error::Validation {
        message: message.into(),
    }
}
fn validate_graph(d: &Definition) -> Result<(), Error> {
    if d.blocks.len() > 100 || d.connections.len() > 100 {
        return Err(validation("Limit is 100 AI blocks and connections."));
    }
    let mut ids = std::collections::HashSet::new();
    for b in &d.blocks {
        if b.id.is_empty()
            || b.id.len() > 64
            || !b
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
            || !ids.insert(&b.id)
            || b.name.trim().is_empty()
            || b.name.len() > 200
            || b.prompt.len() > 65536
            || !b.position.x.is_finite()
            || !b.position.y.is_finite()
            || b.position.x.abs() > 1e6
            || b.position.y.abs() > 1e6
        {
            return Err(validation("Each AI block needs a unique valid identity, name (1–200 bytes), prompt (at most 64 KiB) and finite position."));
        }
    }
    let mut incoming = std::collections::HashMap::new();
    let mut outgoing = std::collections::HashMap::new();
    for c in &d.connections {
        if !ids.contains(&c.source) || !ids.contains(&c.target) || c.source == c.target {
            return Err(validation(
                "Connection needs two different existing AI blocks.",
            ));
        }
        if incoming.insert(&c.target, &c.source).is_some()
            || outgoing.insert(&c.source, &c.target).is_some()
        {
            return Err(validation(
                "Unsupported branching or duplicate connection; use a sequential chain.",
            ));
        }
    }
    for start in &d.blocks {
        let mut visited = std::collections::HashSet::new();
        let mut cursor = &start.id;
        while let Some(next) = outgoing.get(cursor) {
            if !visited.insert(cursor) {
                return Err(validation(
                    "Unsupported cyclic graph; use a sequential chain.",
                ));
            }
            cursor = next;
        }
    }
    if !d.blocks.is_empty() && d.connections.len() != d.blocks.len() - 1 {
        return Err(validation("Disconnected AI blocks cannot execute. Connect a single sequential chain before saving."));
    }
    Ok(())
}
pub struct Store {
    path: PathBuf,
}
impl Store {
    pub fn open(directory: &Path) -> Result<Self, Error> {
        std::fs::create_dir_all(directory).map_err(storage)?;
        private_path(directory, true)?;
        let path = directory.join("definitions.sqlite3");
        if path.exists() || path.symlink_metadata().is_ok() {
            private_path(&path, false)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(&path).map_err(storage)?;
        let mut connection = Connection::open(&path).map_err(storage)?;
        connection
            .busy_timeout(std::time::Duration::from_millis(250))
            .map_err(storage)?;
        let version: u32 = connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(storage)?;
        let tables: u32 = connection.query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'", [], |r| r.get(0)).map_err(storage)?;
        if version > 1 || (version == 0 && tables != 0) {
            let backup = directory.join(format!("backup-{}.sqlite3", uuid::Uuid::new_v4()));
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            options.open(&backup).map_err(storage)?;
            connection
                .backup("main", &backup, None::<fn(rusqlite::backup::Progress)>)
                .map_err(storage)?;
            return Err(Error::UnsupportedSchema { message: "Unsupported desktop schema; a private SQLite backup was created next to the database. Use a compatible app version. No migration was applied.".into() });
        }
        if version == 0 {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            tx.execute_batch("CREATE TABLE revisions (id TEXT NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL, PRIMARY KEY(id,revision)); PRAGMA user_version = 1;").map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        Ok(Self { path })
    }

    pub fn list(&self) -> Result<Vec<SavedDefinition>, Error> {
        let connection = Connection::open(&self.path).map_err(storage)?;
        let mut query = connection.prepare("SELECT r.revision,r.body FROM revisions r WHERE r.revision=(SELECT MAX(s.revision) FROM revisions s WHERE s.id=r.id) ORDER BY r.id LIMIT 501").map_err(storage)?;
        let records: Vec<SavedDefinition> = query
            .query_map([], |row| {
                Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(storage)?
            .map(|row| {
                let (revision, body) = row.map_err(storage)?;
                Ok(SavedDefinition {
                    revision,
                    definition: serde_json::from_str(&body).map_err(storage)?,
                })
            })
            .collect::<Result<_, Error>>()?;
        if records.len() > 500 {
            return Err(Error::Validation { message: "Desktop definition list exceeds the current 500-record limit. No records were deleted.".into() });
        }
        Ok(records)
    }

    pub fn load(&self, id: &str) -> Result<SavedDefinition, Error> {
        if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
            return Err(Error::Validation {
                message: "Definition identity must be a UUID.".into(),
            });
        }
        let connection = Connection::open(&self.path).map_err(storage)?;
        let row: Option<(u32, String)> = connection
            .query_row(
                "SELECT revision,body FROM revisions WHERE id=?1 ORDER BY revision DESC LIMIT 1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let (revision, body) = row.ok_or_else(|| Error::NotFound {
            message: "Definition not found.".into(),
        })?;
        Ok(SavedDefinition {
            revision,
            definition: serde_json::from_str(&body).map_err(storage)?,
        })
    }

    pub fn save(&self, request: SaveDefinition) -> Result<SavedDefinition, Error> {
        if request.definition.schema_version != 1
            || request.definition.id.len() != 36
            || uuid::Uuid::parse_str(&request.definition.id).is_err()
            || request.definition.name.trim().is_empty()
            || request.definition.name.len() > 200
        {
            return Err(Error::Validation {
                message: "Use schema 1, a UUID identity and a name of 1–200 bytes.".into(),
            });
        }
        validate_graph(&request.definition)?;
        let mut connection = Connection::open(&self.path).map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current: Option<u32> = tx
            .query_row(
                "SELECT MAX(revision) FROM revisions WHERE id=?1",
                [&request.definition.id],
                |row| row.get(0),
            )
            .map_err(storage)?;
        if current != request.expected_revision {
            return Err(Error::Conflict { message: "Saved revision changed. Keep your edits or explicitly discard and reopen the saved definition.".into() });
        }
        let saved = SavedDefinition {
            definition: request.definition,
            revision: current
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| storage("revision exhausted"))?,
        };
        let body = serde_json::to_string(&saved.definition).map_err(storage)?;
        tx.execute(
            "INSERT INTO revisions(id,revision,body) VALUES (?1,?2,?3)",
            params![saved.definition.id, saved.revision, body],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(saved)
    }
}
