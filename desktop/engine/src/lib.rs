pub mod auth;
pub mod auth_file;
pub mod output;
pub mod prompts;
pub mod runs;
pub mod runtime;
pub mod sandbox;
pub mod sessions;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum IsolationMode {
    #[default]
    PerRun,
    PerBlock,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Definition {
    pub schema_version: u32,
    #[serde(default)]
    pub isolation: IsolationMode,
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub blocks: Vec<AiBlock>,
    #[serde(default)]
    pub connections: Vec<GraphConnection>,
    #[serde(default)]
    pub inputs: Vec<WorkflowInput>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkflowInput {
    pub name: String,
    pub required: bool,
    pub default: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreparedInputs {
    pub values: std::collections::BTreeMap<String, String>,
    pub prompts: std::collections::BTreeMap<String, String>,
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
    #[serde(default)]
    pub configuration_id: Option<String>,
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
    let mut input_names = std::collections::HashSet::new();
    if d.inputs.len() > 100 {
        return Err(validation("Limit is 100 workflow inputs."));
    }
    for input in &d.inputs {
        let mut chars = input.name.bytes();
        if !chars
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
            || input.name.len() > 64
            || !chars.all(|c| c.is_ascii_alphanumeric() || c == b'_')
            || !input_names.insert(&input.name)
            || input.default.as_ref().is_some_and(|s| s.len() > 65536)
        {
            return Err(validation("Invalid or duplicate input name. Use [A-Za-z_][A-Za-z0-9_]*, at most 64 bytes; defaults at most 64 KiB."));
        }
    }

    for b in &d.blocks {
        substitute(&b.prompt, |name| {
            if input_names.contains(&name.to_string()) {
                Ok(String::new())
            } else {
                Err(validation(
                    "Dangling input reference; declare the input before saving.",
                ))
            }
        })?;
    }
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
fn substitute(
    prompt: &str,
    mut lookup: impl FnMut(&str) -> Result<String, Error>,
) -> Result<String, Error> {
    let mut rest = prompt;
    let mut result = String::new();
    while let Some(index) = rest.find("{{") {
        result.push_str(&rest[..index]);
        let tail = &rest[index + 2..];
        let end = tail
            .find("}}")
            .ok_or_else(|| validation("Unclosed input reference."))?;
        let token = &tail[..end];
        let value = if let Some(name) = token.strip_prefix("input.") {
            lookup(name)?
        } else if token.starts_with("output.") && token.ends_with(".finalResponse") {
            format!("{{{{{token}}}}}")
        } else {
            return Err(validation(
                "Use {{input.name}} or {{output.blockId.finalResponse}}.",
            ));
        };
        if result
            .len()
            .saturating_add(value.len())
            .saturating_add(tail.len() - end - 2)
            > 1_048_576
        {
            return Err(validation(
                "Prepared prompt exceeds 1 MiB; shorten input values or references.",
            ));
        }
        result.push_str(&value);
        rest = &tail[end + 2..];
    }
    result.push_str(rest);
    Ok(result)
}
pub fn resolve_inputs(
    d: &Definition,
    supplied: std::collections::BTreeMap<String, String>,
) -> Result<PreparedInputs, Error> {
    validate_graph(d)?;
    let declared = d
        .inputs
        .iter()
        .map(|i| (i.name.clone(), String::new()))
        .collect();
    for block in &d.blocks {
        prompts::compile(d, &block.id, &declared)?;
    }
    if supplied.len() > 100
        || supplied
            .iter()
            .any(|(k, v)| v.len() > 65536 || !d.inputs.iter().any(|i| &i.name == k))
    {
        return Err(validation("Unknown input or input value exceeds 64 KiB."));
    }
    for input in &d.inputs {
        if input.required
            && supplied
                .get(&input.name)
                .or(input.default.as_ref())
                .is_none_or(|v| v.is_empty())
        {
            return Err(validation(&format!(
                "Required input '{}' needs a supplied value or default.",
                input.name
            )));
        }
    }
    let mut values = std::collections::BTreeMap::new();
    for input in &d.inputs {
        values.insert(
            input.name.clone(),
            supplied
                .get(&input.name)
                .cloned()
                .or_else(|| input.default.clone())
                .unwrap_or_default(),
        );
    }
    let mut prompts = std::collections::BTreeMap::new();
    for b in &d.blocks {
        prompts.insert(
            b.id.clone(),
            substitute(&b.prompt, |name| {
                values
                    .get(name)
                    .cloned()
                    .ok_or_else(|| validation("Dangling input reference."))
            })?,
        );
    }
    Ok(PreparedInputs { values, prompts })
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexConfiguration {
    pub id: String,
    pub name: String,
    pub model: String,
    pub reasoning: Reasoning,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum Reasoning {
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedConfiguration {
    pub config: CodexConfiguration,
    pub revision: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveConfiguration {
    pub config: CodexConfiguration,
    pub expected_revision: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum CredentialSourceKind {
    NotInspected,
    CodexFile,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum ContainerAuthSupport {
    UnsupportedConcurrentAuth,
}
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthReadiness {
    pub execution_enabled: bool,
    pub credential_source: CredentialSourceKind,
    pub credential_availability: auth::CredentialAvailability,
    pub status: ContainerAuthSupport,
    pub guidance: String,
}
pub fn auth_readiness() -> AuthReadiness {
    credential_readiness(auth::CredentialAvailability::NotInspected)
}
pub fn credential_readiness(availability: auth::CredentialAvailability) -> AuthReadiness {
    use auth::CredentialAvailability as A;
    let setup = match availability {
        A::NotInspected => "No credential inspection requested.",
        A::Available => "Subscription-shaped material is available in the supported existing Codex file. Validity, expiry, revocation and entitlement are not checked; this is not authentication.",
        A::Missing => "The fixed Codex file is missing. This does not rule out a keyring login. Consult Codex's own subscription setup; do not export or convert credentials for this app.",
        A::Unsupported => "This source or auth kind is unsupported. Keyring, custom CODEX_HOME, ephemeral, external tokens and API keys are not inspected; do not change stores for this app.",
        A::Malformed => "The supported file does not match the bounded subscription schema. Use Codex's own setup guidance; this app will not repair it.",
        A::Inaccessible => "The fixed Codex source could not be read. Review local access using Codex's own setup; no fallback was attempted.",
        A::Unsafe => "Source safety checks failed (type, ownership, permissions, links, size or concurrent modification). No repair, copying or fallback was attempted.",
    };
    AuthReadiness { execution_enabled:false, credential_source:if availability==A::NotInspected {CredentialSourceKind::NotInspected} else {CredentialSourceKind::CodexFile}, credential_availability:availability, status:ContainerAuthSupport::UnsupportedConcurrentAuth, guidance:format!("{setup} Execution blocked: this host-file inspector does not authorize the separate app-owned managed-session route. That gated source route still requires trusted-private workload authorization and native secure-store, staging, paging/dump and provider-containment qualification in #29. No login, refresh, host-auth write, paid API-key fallback or model call is performed.") }
}
fn validate_configuration(c: &CodexConfiguration) -> Result<(), Error> {
    if c.id.len() != 36
        || uuid::Uuid::parse_str(&c.id).is_err()
        || c.name.trim().is_empty()
        || c.name.len() > 200
        || c.model.is_empty()
        || c.model.len() > 100
        || !c
            .model
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    {
        return Err(validation("Configuration needs a UUID, name (1–200 bytes), model identifier (1–100 ASCII letters/digits/-_.) and supported reasoning setting. Never enter credentials."));
    }
    Ok(())
}
fn validate_definition(d: &Definition) -> Result<(), Error> {
    if d.schema_version != 1
        || d.id.len() != 36
        || uuid::Uuid::parse_str(&d.id).is_err()
        || d.name.trim().is_empty()
        || d.name.len() > 200
    {
        return Err(Error::Validation {
            message: "Use schema 1, a UUID identity and a name of 1–200 bytes.".into(),
        });
    }
    validate_graph(d)?;
    for b in &d.blocks {
        let declared = d
            .inputs
            .iter()
            .map(|i| (i.name.clone(), String::new()))
            .collect();
        prompts::compile(d, &b.id, &declared)?;
        if let Some(id) = &b.configuration_id {
            if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
                return Err(validation("Invalid configuration identity."));
            }
        }
    }
    Ok(())
}
fn stored_definition(id: &str, revision: u32, body: &str) -> Result<SavedDefinition, Error> {
    let definition: Definition = serde_json::from_str(body).map_err(storage)?;
    if definition.id != id || revision == 0 {
        return Err(storage("stored identity/revision"));
    }
    validate_definition(&definition).map_err(|_| storage("invalid stored definition"))?;
    Ok(SavedDefinition {
        definition,
        revision,
    })
}
fn stored_configuration(id: &str, revision: u32, body: &str) -> Result<SavedConfiguration, Error> {
    let config: CodexConfiguration = serde_json::from_str(body).map_err(storage)?;
    if config.id != id || revision == 0 {
        return Err(storage("stored identity/revision"));
    }
    validate_configuration(&config).map_err(|_| storage("invalid stored configuration"))?;
    Ok(SavedConfiguration { config, revision })
}
fn read_configuration(
    connection: &Connection,
    id: &str,
) -> Result<Option<SavedConfiguration>, Error> {
    let row: Option<(u32, String)> = connection
        .query_row(
            "SELECT revision,body FROM configurations WHERE id=?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(storage)?;
    row.map(|(revision, body)| stored_configuration(id, revision, &body))
        .transpose()
}
fn enforce_capacity(connection: &Connection, table: &str) -> Result<(), Error> {
    let sql = match table {
        "revisions" => "SELECT COUNT(DISTINCT id) FROM revisions",
        _ => "SELECT COUNT(*) FROM configurations",
    };
    let count: u32 = connection
        .query_row(sql, [], |r| r.get(0))
        .map_err(storage)?;
    if count >= 500 {
        return Err(validation(
            "Limit is 500 records. Existing records remain accessible and editable.",
        ));
    }
    Ok(())
}
pub mod lifecycle;
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
        if version > 4 || (version == 0 && tables != 0) {
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
        if version <= 1 {
            let tx = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            tx.execute_batch("CREATE TABLE configurations (id TEXT PRIMARY KEY NOT NULL, revision INTEGER NOT NULL, body TEXT NOT NULL); PRAGMA user_version=2;").map_err(storage)?;
            tx.commit().map_err(storage)?;
        }
        if version <= 2 {
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE runs(id TEXT PRIMARY KEY,request_id TEXT UNIQUE NOT NULL,request TEXT NOT NULL,snapshot TEXT NOT NULL,status TEXT NOT NULL,active INTEGER NOT NULL); CREATE UNIQUE INDEX one_active_run ON runs(active) WHERE active=1; CREATE TABLE managed_sessions(id TEXT PRIMARY KEY,state TEXT NOT NULL,generation INTEGER NOT NULL,owner TEXT); PRAGMA user_version=3; COMMIT;").map_err(storage)?;
        }
        if version <= 3 {
            connection.execute_batch("BEGIN IMMEDIATE; CREATE TABLE queue_settings(id INTEGER PRIMARY KEY CHECK(id=1),active INTEGER NOT NULL CHECK(active BETWEEN 1 AND 8),waiting INTEGER NOT NULL CHECK(waiting BETWEEN 0 AND 64)); INSERT INTO queue_settings VALUES(1,2,16); CREATE TABLE run_queue(sequence INTEGER PRIMARY KEY AUTOINCREMENT,run_id TEXT UNIQUE NOT NULL,phase TEXT NOT NULL CHECK(phase IN ('waiting','occupied','settled','withheld')),slot TEXT,generation INTEGER); INSERT INTO run_queue(run_id,phase,slot,generation) SELECT id,CASE WHEN active=1 THEN 'occupied' ELSE 'settled' END,(SELECT s.id FROM managed_sessions s WHERE s.owner=runs.id LIMIT 1),(SELECT s.generation FROM managed_sessions s WHERE s.owner=runs.id LIMIT 1) FROM runs ORDER BY rowid; DROP INDEX IF EXISTS one_active_run; PRAGMA user_version=4; COMMIT;").map_err(storage)?;
        }
        connection.execute_batch("CREATE TABLE IF NOT EXISTS run_resources(run_id TEXT NOT NULL,kind TEXT NOT NULL,resource_id TEXT,PRIMARY KEY(run_id,kind));").map_err(storage)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS run_recovery(run_id TEXT PRIMARY KEY NOT NULL,slot TEXT NOT NULL,generation INTEGER NOT NULL,resources TEXT,finality TEXT);").map_err(storage)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS desktop_lifecycle(id INTEGER PRIMARY KEY CHECK(id=1),frozen INTEGER NOT NULL); INSERT OR IGNORE INTO desktop_lifecycle VALUES(1,0);").map_err(storage)?;
        Ok(Self { path })
    }

    pub fn delete_configuration(&self, id: &str, expected_revision: u32) -> Result<(), Error> {
        if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
            return Err(validation("Configuration identity must be a UUID."));
        }
        let mut connection = Connection::open(&self.path).map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        read_configuration(&tx, id)?.ok_or_else(|| Error::NotFound {
            message: "Configuration not found.".into(),
        })?;
        {
            let mut query=tx.prepare("SELECT r.id,r.revision,r.body FROM revisions r WHERE r.revision=(SELECT MAX(s.revision) FROM revisions s WHERE s.id=r.id)").map_err(storage)?;
            for body in query
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, u32>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })
                .map_err(storage)?
            {
                let (row_id, revision, body) = body.map_err(storage)?;
                let d = stored_definition(&row_id, revision, &body)?.definition;
                if d.blocks
                    .iter()
                    .any(|b| b.configuration_id.as_deref() == Some(id))
                {
                    return Err(validation("Configuration is referenced by a saved definition. Remove the reference and save first."));
                }
            }
        }
        if tx
            .execute(
                "DELETE FROM configurations WHERE id=?1 AND revision=?2",
                params![id, expected_revision],
            )
            .map_err(storage)?
            != 1
        {
            return Err(Error::Conflict {
                message: "Configuration changed or was removed; reload before deleting.".into(),
            });
        }
        tx.commit().map_err(storage)?;
        Ok(())
    }
    pub fn list_configurations(&self) -> Result<Vec<SavedConfiguration>, Error> {
        let connection = Connection::open(&self.path).map_err(storage)?;
        let mut query = connection
            .prepare("SELECT id,revision,body FROM configurations ORDER BY id LIMIT 501")
            .map_err(storage)?;
        let records = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?
            .map(|row| {
                let (id, revision, body) = row.map_err(storage)?;
                stored_configuration(&id, revision, &body)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        if records.len() > 500 {
            return Err(validation("Configuration limit is 500."));
        }
        Ok(records)
    }
    pub fn save_configuration(
        &self,
        request: SaveConfiguration,
    ) -> Result<SavedConfiguration, Error> {
        let c = &request.config;
        validate_configuration(c)?;
        let mut connection = Connection::open(&self.path).map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let current = read_configuration(&tx, &c.id)?.map(|saved| saved.revision);
        if current.is_none() {
            enforce_capacity(&tx, "configurations")?;
        }
        if current != request.expected_revision {
            return Err(Error::Conflict {
                message: "Configuration changed. Keep edits or explicitly reload.".into(),
            });
        }
        let revision = current
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| storage("revision exhausted"))?;
        let body = serde_json::to_string(c).map_err(storage)?;
        tx.execute("INSERT INTO configurations(id,revision,body) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET revision=excluded.revision,body=excluded.body",params![c.id,revision,body]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(SavedConfiguration {
            config: request.config,
            revision,
        })
    }
    pub fn list(&self) -> Result<Vec<SavedDefinition>, Error> {
        let connection = Connection::open(&self.path).map_err(storage)?;
        let mut query = connection.prepare("SELECT r.id,r.revision,r.body FROM revisions r WHERE r.revision=(SELECT MAX(s.revision) FROM revisions s WHERE s.id=r.id) ORDER BY r.id LIMIT 501").map_err(storage)?;
        let records: Vec<SavedDefinition> = query
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, u32>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(storage)?
            .map(|row| {
                let (id, revision, body) = row.map_err(storage)?;
                stored_definition(&id, revision, &body)
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
        stored_definition(id, revision, &body)
    }

    pub fn save(&self, request: SaveDefinition) -> Result<SavedDefinition, Error> {
        validate_definition(&request.definition)?;
        let mut connection = Connection::open(&self.path).map_err(storage)?;
        let tx = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        for b in &request.definition.blocks {
            if let Some(id) = &b.configuration_id {
                if read_configuration(&tx, id)?.is_none() {
                    return Err(validation("Missing or invalid Codex configuration reference. Select an existing configuration."));
                }
            }
        }
        let row: Option<(u32, String)> = tx
            .query_row(
                "SELECT revision,body FROM revisions WHERE id=?1 ORDER BY revision DESC LIMIT 1",
                [&request.definition.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let current = row
            .map(|(revision, body)| {
                stored_definition(&request.definition.id, revision, &body)
                    .map(|saved| saved.revision)
            })
            .transpose()?;
        if current.is_none() {
            enforce_capacity(&tx, "revisions")?;
        }
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
