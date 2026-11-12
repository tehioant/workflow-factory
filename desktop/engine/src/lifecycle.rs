//! Short SQLite authority transitions, never external I/O or dialogs.
use crate::{storage, validation, Error, Store, TransactionBehavior};
use rusqlite::Connection;
pub(crate) fn frozen(c: &Connection) -> Result<bool, Error> {
    c.query_row("SELECT frozen FROM desktop_lifecycle WHERE id=1", [], |r| {
        r.get(0)
    })
    .map_err(storage)
}
pub(crate) fn admit(c: &Connection) -> Result<(), Error> {
    if frozen(c)? {
        return Err(validation(
            "Shutdown admission is frozen. Keep the app open for selected recovery.",
        ));
    }
    Ok(())
}
pub(crate) fn snapshot(c: &Connection) -> Result<(String, u32), Error> {
    let mut values = Vec::new();
    // Lazy byte/JSON validation precedes JSON extraction. Historical output is
    // never materialized; only activity and ownership affect consent.
    for sql in [
        "SELECT CASE WHEN length(CAST(id AS BLOB))<=128 AND length(CAST(status AS BLOB))<=2097152 THEN CASE WHEN json_valid(status) THEN CASE WHEN json_extract(status,'$.id')=id AND json_extract(status,'$.state') IN ('queued','authorizationRequired','admitted','running','cancelling','held','succeeded','failed','stopped','blocked') AND json_type(status,'$.cleanupPending') IN ('true','false') THEN json_array(id,active,json_extract(status,'$.id'),json_extract(status,'$.state'),json_extract(status,'$.cleanupPending')) END END END FROM runs WHERE active!=0 OR CASE WHEN length(CAST(status AS BLOB))<=2097152 THEN CASE WHEN json_valid(status) THEN COALESCE(json_extract(status,'$.cleanupPending'),1)!=0 OR COALESCE(json_extract(status,'$.state'),'') NOT IN ('succeeded','failed','stopped','blocked') OR COALESCE(json_extract(status,'$.id'),'')!=id ELSE 1 END ELSE 1 END ORDER BY id LIMIT 513",
        "SELECT CASE WHEN length(CAST(run_id AS BLOB))+COALESCE(length(CAST(slot AS BLOB)),0)<=256 AND phase IN ('waiting','occupied') THEN json_array(run_id,phase,slot,generation) END FROM run_queue WHERE phase!='settled' ORDER BY sequence LIMIT 513",
        "SELECT CASE WHEN length(CAST(id AS BLOB))+COALESCE(length(CAST(owner AS BLOB)),0)<=256 THEN json_array(id,state,generation,owner) END FROM managed_sessions WHERE state IN ('leased','acquiring') ORDER BY id LIMIT 513",
        "SELECT CASE WHEN length(CAST(run_id AS BLOB))+length(CAST(kind AS BLOB))+COALESCE(length(CAST(resource_id AS BLOB)),0)<=1024 THEN json_array(run_id,kind,resource_id) END FROM run_resources WHERE run_id IN (SELECT id FROM runs WHERE active!=0) OR NOT EXISTS(SELECT 1 FROM run_queue WHERE run_id=run_resources.run_id AND phase='settled') ORDER BY run_id,kind LIMIT 513",
    ] {
        let mut q = c.prepare(sql).map_err(storage)?;
        let rows = q.query_map([], |r| r.get::<_, Option<String>>(0))
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
        if rows.len()>512 || rows.iter().any(Option::is_none) {
            return Err(validation("Shutdown state unavailable or exceeds safe bounds."));
        }
        values.push(rows);
    }
    let work = values[0].len() as u32;
    Ok((serde_json::to_string(&values).map_err(storage)?, work))
}
impl Store {
    /// Called only on startup after exclusive data ownership and restart withholding.
    pub fn reset_shutdown(&self) -> Result<(), Error> {
        Connection::open(&self.path)
            .map_err(storage)?
            .execute("UPDATE desktop_lifecycle SET frozen=0 WHERE id=1", [])
            .map_err(storage)?;
        Ok(())
    }
    pub fn shutdown_snapshot(&self) -> Result<String, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c.transaction().map_err(storage)?;
        snapshot(&tx).map(|value| value.0)
    }
    /// Compare and freeze serialize with every engine admission/reservation effect.
    pub fn freeze_shutdown(&self, expected: &str) -> Result<Vec<String>, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if snapshot(&tx)?.0 != expected {
            return Err(validation(
                "Work changed. Recheck shutdown and choose again.",
            ));
        }
        tx.execute("UPDATE desktop_lifecycle SET frozen=1 WHERE id=1", [])
            .map_err(storage)?;
        let ids = {
            let mut q = tx
                .prepare("SELECT id FROM runs WHERE active=1 ORDER BY id")
                .map_err(storage)?;
            let ids = q
                .query_map([], |r| r.get(0))
                .map_err(storage)?
                .collect::<Result<Vec<String>, _>>()
                .map_err(storage)?;
            ids
        };
        tx.commit().map_err(storage)?;
        Ok(ids)
    }
    /// A settled occupied lease can only be released by the existing positive receipt.
    pub fn shutdown_ready(&self) -> Result<bool, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c.transaction().map_err(storage)?;
        if !frozen(&tx)? {
            return Ok(false);
        }
        let (body, _) = snapshot(&tx)?;
        let rows: Vec<Vec<String>> = serde_json::from_str(&body).map_err(storage)?;
        let orphan:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM run_queue q WHERE NOT EXISTS(SELECT 1 FROM runs r WHERE r.id=q.run_id) OR (q.slot IS NULL)!=(q.generation IS NULL) OR (q.slot IS NULL AND EXISTS(SELECT 1 FROM run_resources rr WHERE rr.run_id=q.run_id)))",[],|r|r.get(0)).map_err(storage)?;
        Ok(!orphan && rows.iter().all(Vec::is_empty))
    }
}

#[derive(serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
pub struct ShutdownPreview {
    pub token: String,
    pub work: u32,
    pub workers: u32,
    pub setup: bool,
    pub retained: bool,
    pub frozen: bool,
}
