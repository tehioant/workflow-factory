//! Bounded, read-only projections of captured rows. No execution or recovery dependencies.
use super::{RunState, RunStatus, Snapshot, StartRun};
use crate::{params, storage, validation, Error, OptionalExtension, Store};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use ts_rs::TS;

const MAX_ROW: usize = 2 * 1024 * 1024;
const SCOPE: &str =
    "CASE WHEN length(CAST(snapshot AS BLOB))<=2097152 THEN CASE WHEN json_valid(snapshot) THEN json_extract(snapshot,'$.definition.definition.id') END END";
// Return the exact stored ID or NULL, never a truncated or normalized identity.
const SAFE_ID: &str = "CASE WHEN length(CAST(id AS BLOB))=36 THEN CASE WHEN substr(id,9,1)='-' AND substr(id,14,1)='-' AND substr(id,19,1)='-' AND substr(id,24,1)='-' AND length(replace(id,'-',''))=32 AND id NOT GLOB '*[^0-9a-fA-F-]*' THEN id END END";

#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub state: Option<RunState>,
    pub input_summary: String,
    pub unavailable: bool,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct HistoryPage {
    pub definition_id: String,
    pub cursor: Option<String>,
    pub entries: Vec<HistoryEntry>,
    pub next_cursor: Option<String>,
    pub unavailable_records: bool,
    pub unknown_scope_records: bool,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CapturedBlock {
    pub block_id: String,
    pub configuration: Option<crate::SavedConfiguration>,
    pub result: Option<super::BlockResult>,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunDetail {
    pub id: String,
    pub definition_id: String,
    pub definition: crate::SavedDefinition,
    pub inputs: crate::PreparedInputs,
    pub image_id: String,
    pub state: RunState,
    pub cleanup_pending: bool,
    pub diagnostic: String,
    pub final_response: Option<String>,
    pub legacy: bool,
    pub legacy_configuration: Option<crate::SavedConfiguration>,
    pub blocks: Vec<CapturedBlock>,
}
fn identity(id: &str) -> Result<(), Error> {
    if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
        return Err(validation("Invalid history identity."));
    }
    Ok(())
}
fn unavailable() -> Error {
    validation("Captured run unavailable: unsupported, incomplete, oversized or inconsistent retained record.")
}
/// Retains the actual SQLite connection and checked handles. These checks detect
/// path drift, not arbitrary ancestor races.
pub struct HistoryReader {
    c: Connection,
    root: std::path::PathBuf,
    parent: std::fs::File,
    db: std::fs::File,
}
impl HistoryReader {
    pub fn shutdown_snapshot(&self) -> Result<(String, u32), Error> {
        let tx = self.checked()?.unchecked_transaction().map_err(storage)?;
        crate::lifecycle::snapshot(&tx)
    }
    fn checked(&self) -> Result<&Connection, Error> {
        use std::os::unix::fs::MetadataExt;
        for (handle, path, directory) in [
            (&self.parent, self.root.clone(), true),
            (&self.db, self.root.join("definitions.sqlite3"), false),
        ] {
            super::inspection::private(handle, directory)?;
            let held = handle.metadata().map_err(storage)?;
            let named = path.symlink_metadata().map_err(storage)?;
            if named.file_type().is_symlink()
                || held.dev() != named.dev()
                || held.ino() != named.ino()
            {
                return Err(unavailable());
            }
        }
        Ok(&self.c)
    }
    pub fn queue_status(&self) -> Result<super::queue::QueueStatus, Error> {
        super::queue::projection(self.checked()?)
    }
    pub fn run_status(&self, id: Option<&str>) -> Result<Option<RunStatus>, Error> {
        if let Some(id) = id {
            identity(id)?;
        }
        let tx = self.checked()?.unchecked_transaction().map_err(storage)?;
        let sql = format!("SELECT {SAFE_ID}, {SCOPE} FROM runs WHERE (?1 IS NULL OR id=?1) ORDER BY rowid DESC LIMIT 1");
        let row: Option<(Option<String>, Option<String>)> = tx
            .query_row(&sql, [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(storage)?;
        let Some((Some(id), Some(workflow))) = row else {
            return if row.is_none() && id.is_none() {
                Ok(None)
            } else {
                Err(unavailable())
            };
        };
        identity(&workflow)?;
        let (_, status) = validated_capture(&tx, &workflow, &id)?;
        Ok(Some(status))
    }
    pub fn workflow(&self, id: &str) -> Result<String, Error> {
        identity(id)?;
        let c = self.checked()?;
        let sql = format!("SELECT {SCOPE} FROM runs WHERE id=?1");
        let workflow: String = c.query_row(&sql, [id], |r| r.get(0)).map_err(storage)?;
        identity(&workflow)?;
        detail(c, &workflow, id)?;
        Ok(workflow)
    }
    pub fn open(root: &std::path::Path) -> Result<Self, Error> {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let parent = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)
            .map_err(storage)?;
        super::inspection::private(&parent, true)?;
        let db = super::inspection::open(
            &parent,
            "definitions.sqlite3",
            libc::O_RDONLY | libc::O_NONBLOCK,
        )?;
        super::inspection::private(&db, false)?;
        // SQLite NOFOLLOW rejects symlinks in *any* component (including macOS
        // /var). Resolve only the already checked private root, never an unchecked
        // database or /dev/fd path, then retain NOFOLLOW on SQLite's actual open.
        let resolved_root = root.canonicalize().map_err(storage)?;
        let named_parent = resolved_root.metadata().map_err(storage)?;
        let held_parent = parent.metadata().map_err(storage)?;
        if named_parent.dev() != held_parent.dev() || named_parent.ino() != held_parent.ino() {
            return Err(unavailable());
        }
        let c = Connection::open_with_flags(
            resolved_root.join("definitions.sqlite3"),
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )
        .map_err(storage)?;
        c.busy_timeout(std::time::Duration::from_millis(250))
            .map_err(storage)?;
        let version: u32 = c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(storage)?;
        if !(3..=4).contains(&version) {
            return Err(unavailable());
        }
        // Confirm SQLite's resolved name still identifies the checked descriptor.
        let actual = c.path().ok_or_else(unavailable)?;
        let named = std::fs::metadata(actual).map_err(storage)?;
        let held = db.metadata().map_err(storage)?;
        if named.dev() != held.dev() || named.ino() != held.ino() {
            return Err(unavailable());
        }
        let reader = Self {
            c,
            root: root.to_path_buf(),
            parent,
            db,
        };
        reader.checked()?;
        Ok(reader)
    }
    pub fn run_detail(&self, workflow: &str, id: &str) -> Result<RunDetail, Error> {
        identity(workflow)?;
        identity(id)?;
        let tx = self.checked()?.unchecked_transaction().map_err(storage)?;
        detail(&tx, workflow, id)
    }
    pub fn list_run_history(
        &self,
        workflow: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<HistoryPage, Error> {
        list(self.checked()?, workflow, cursor, limit)
    }
}
fn reader(store: &Store) -> Result<HistoryReader, Error> {
    HistoryReader::open(store.path.parent().ok_or_else(unavailable)?)
}
fn validated_capture(
    c: &Connection,
    workflow: &str,
    id: &str,
) -> Result<(Snapshot, RunStatus), Error> {
    // Check bytes in SQL before materializing any stored JSON in Rust.
    let sql = format!("SELECT request_id,request,snapshot,status FROM runs WHERE id=?1 AND {SCOPE}=?2 AND length(CAST(request_id AS BLOB))=36 AND length(CAST(request AS BLOB))+length(CAST(snapshot AS BLOB))+length(CAST(status AS BLOB))<=?3");
    let row: Option<(String, String, String, String)> = c
        .query_row(&sql, params![id, workflow, MAX_ROW as i64], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()
        .map_err(storage)?;
    let (request_id, request, snapshot, status) = row.ok_or_else(unavailable)?;
    let request: StartRun = serde_json::from_str(&request).map_err(|_| unavailable())?;
    let snapshot: Snapshot = serde_json::from_str(&snapshot).map_err(|_| unavailable())?;
    let status: RunStatus = serde_json::from_str(&status).map_err(|_| unavailable())?;
    identity(&request_id)?;
    let d = &snapshot.definition.definition;
    if request_id != request.request_id
        || request_id != status.request_id
        || status.id != id
        || request.definition_id != workflow
        || d.id != workflow
        || request.expected_revision != snapshot.definition.revision
        || snapshot.definition.revision == 0
    {
        return Err(unavailable());
    }
    crate::validate_definition(d).map_err(|_| unavailable())?;
    let valid_config = |config: &crate::SavedConfiguration| -> Result<(), Error> {
        if config.revision == 0 {
            return Err(unavailable());
        }
        crate::validate_configuration(&config.config).map_err(|_| unavailable())
    };
    valid_config(&snapshot.configuration)?;
    if request.expected_configuration_revisions.is_empty()
        && snapshot.configuration.revision != request.expected_configuration_revision
    {
        return Err(unavailable());
    }
    // Compare captured values/prompts against the captured definition and request, never today's data.
    // Bound expansion before calling legacy input preparation, whose strings can otherwise
    // expand many repeated references far beyond the stored row's byte limit.
    let mut expansion = d.blocks.iter().map(|b| b.prompt.len()).sum::<usize>();
    for block in &d.blocks {
        let mut rest = block.prompt.as_str();
        while let Some(index) = rest.find("{{input.") {
            let tail = &rest[index + 8..];
            let end = tail.find("}}").ok_or_else(unavailable)?;
            let name = &tail[..end];
            let value = request.supplied.get(name).or_else(|| {
                d.inputs
                    .iter()
                    .find(|i| i.name == name)
                    .and_then(|i| i.default.as_ref())
            });
            expansion = expansion.saturating_add(value.map_or(0, String::len));
            if expansion > MAX_ROW {
                return Err(unavailable());
            }
            rest = &tail[end + 2..];
        }
    }
    let prepared = crate::resolve_inputs(d, request.supplied).map_err(|_| unavailable())?;
    if prepared != snapshot.inputs {
        return Err(unavailable());
    }
    let order = crate::prompts::ordered_ids(d).map_err(|_| unavailable())?;
    let legacy = snapshot.plans.is_empty();
    if order.is_empty()
        || (!legacy && snapshot.plans.len() != order.len())
        || status.blocks.len() > order.len()
        || (legacy && !status.blocks.is_empty())
        || snapshot.image_id.is_empty()
        || snapshot.image_id.len() > 200
    {
        return Err(unavailable());
    }
    for (plan, block_id) in snapshot.plans.iter().zip(&order) {
        valid_config(&plan.configuration)?;
        let block = d
            .blocks
            .iter()
            .find(|b| &b.id == block_id)
            .ok_or_else(unavailable)?;
        let expected = request
            .expected_configuration_revisions
            .get(&plan.configuration.config.id)
            .copied()
            .or_else(|| {
                (order.len() == 1 && request.expected_configuration_revisions.is_empty())
                    .then_some(request.expected_configuration_revision)
            });
        if &plan.block_id != block_id
            || block.configuration_id.as_ref() != Some(&plan.configuration.config.id)
            || expected != Some(plan.configuration.revision)
        {
            return Err(unavailable());
        }
        let compiled = crate::prompts::compile(d, block_id, &snapshot.inputs.values)
            .map_err(|_| unavailable())?;
        if serde_json::to_value(&compiled).map_err(storage)?
            != serde_json::to_value(&plan.segments).map_err(storage)?
        {
            return Err(unavailable());
        }
    }
    for (i, result) in status.blocks.iter().enumerate() {
        if result.block_id != order[i]
            || result.resolved_prompt.len() > MAX_ROW
            || (i + 1 < status.blocks.len()
                && (result.state != RunState::Succeeded || result.final_response.is_none()))
            || (result.state == RunState::Succeeded && result.final_response.is_none())
        {
            return Err(unavailable());
        }
    }
    if legacy && order.len() == 1 {
        let block = &d.blocks[0];
        if block.configuration_id.as_ref() != Some(&snapshot.configuration.config.id) {
            return Err(unavailable());
        }
    }
    if !legacy
        && snapshot
            .plans
            .first()
            .is_some_and(|p| p.configuration != snapshot.configuration)
    {
        return Err(unavailable());
    }
    let mut outputs = std::collections::BTreeMap::new();
    for (result, plan) in status.blocks.iter().zip(&snapshot.plans) {
        if !matches!(
            result.state,
            RunState::Running | RunState::Succeeded | RunState::Failed
        ) || crate::prompts::resolve(&plan.segments, &outputs).map_err(|_| unavailable())?
            != result.resolved_prompt
        {
            return Err(unavailable());
        }
        if result.state == RunState::Succeeded {
            outputs.insert(
                result.block_id.clone(),
                result.final_response.clone().ok_or_else(unavailable)?,
            );
        }
    }
    Ok((snapshot, status))
}
pub(crate) fn detail(c: &Connection, workflow: &str, id: &str) -> Result<RunDetail, Error> {
    let (snapshot, status) = validated_capture(c, workflow, id)?;
    let order =
        crate::prompts::ordered_ids(&snapshot.definition.definition).map_err(|_| unavailable())?;
    let legacy = snapshot.plans.is_empty();
    let blocks = order
        .into_iter()
        .map(|block_id| CapturedBlock {
            configuration: snapshot
                .plans
                .iter()
                .find(|p| p.block_id == block_id)
                .map(|p| p.configuration.clone()),
            result: status
                .blocks
                .iter()
                .find(|b| b.block_id == block_id)
                .cloned(),
            block_id,
        })
        .collect();
    Ok(RunDetail {
        id: id.into(),
        definition_id: workflow.into(),
        definition: snapshot.definition,
        inputs: snapshot.inputs,
        image_id: snapshot.image_id,
        state: status.state,
        cleanup_pending: status.cleanup_pending,
        diagnostic: status.diagnostic,
        final_response: status.final_response,
        legacy,
        legacy_configuration: legacy.then_some(snapshot.configuration),
        blocks,
    })
}
impl Store {
    pub fn run_detail(&self, workflow: &str, id: &str) -> Result<RunDetail, Error> {
        identity(workflow)?;
        identity(id)?;
        reader(self)?.run_detail(workflow, id)
    }
    pub fn list_run_history(
        &self,
        workflow: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<HistoryPage, Error> {
        reader(self)?.list_run_history(workflow, cursor, limit)
    }
}
fn list(
    c: &Connection,
    workflow: &str,
    cursor: Option<&str>,
    limit: u32,
) -> Result<HistoryPage, Error> {
    identity(workflow)?;
    if !(1..=50).contains(&limit) {
        return Err(validation("History page size must be 1–50."));
    }
    let tx = c.unchecked_transaction().map_err(storage)?;
    let boundary: Option<i64> = if let Some(id) = cursor {
        identity(id)?;
        let sql =
            format!("SELECT rowid FROM runs WHERE id=?1 AND {SAFE_ID} IS NOT NULL AND {SCOPE}=?2");
        Some(
            tx.query_row(&sql, params![id, workflow], |r| r.get(0))
                .optional()
                .map_err(storage)?
                .ok_or_else(unavailable)?,
        )
    } else {
        None
    };
    let warnings = format!("SELECT EXISTS(SELECT 1 FROM runs WHERE {SCOPE}=?1 AND {SAFE_ID} IS NULL), EXISTS(SELECT 1 FROM runs WHERE {SCOPE} IS NULL)");
    let (unavailable_records, unknown_scope_records) = tx
        .query_row(&warnings, [workflow], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(storage)?;
    let sql = format!("SELECT {SAFE_ID} FROM runs WHERE {SCOPE}=?1 AND {SAFE_ID} IS NOT NULL AND (?2 IS NULL OR rowid<?2) ORDER BY rowid DESC LIMIT ?3");
    let mut query = tx.prepare(&sql).map_err(storage)?;
    let ids = query
        .query_map(params![workflow, boundary, limit + 1], |r| {
            r.get::<_, String>(0)
        })
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    let has_more = ids.len() > limit as usize;
    let mut entries = Vec::new();
    for id in ids.into_iter().take(limit as usize) {
        identity(&id)?;
        let captured = detail(&tx, workflow, &id);
        let entry = match captured {
            Ok(d) => {
                let mut summary = String::new();
                for (name, value) in d.inputs.values {
                    for ch in format!("{name}: {value}; ").chars() {
                        if summary.len() + ch.len_utf8() > 240 {
                            break;
                        }
                        summary.push(ch);
                    }
                    if summary.len() >= 237 {
                        break;
                    }
                }
                HistoryEntry {
                    id,
                    state: Some(d.state),
                    input_summary: summary,
                    unavailable: false,
                }
            }
            Err(_) => HistoryEntry {
                id,
                state: None,
                input_summary: "Captured data unavailable".into(),
                unavailable: true,
            },
        };
        entries.push(entry);
    }
    let next_cursor = has_more.then(|| entries.last().unwrap().id.clone());
    Ok(HistoryPage {
        definition_id: workflow.into(),
        cursor: cursor.map(str::to_string),
        entries,
        next_cursor,
        unavailable_records,
        unknown_scope_records,
    })
}
