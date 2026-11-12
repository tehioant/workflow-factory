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
fn snapshot(c: &Connection) -> Result<String, Error> {
    let mut values = Vec::new();
    for sql in [
        "SELECT json_array(id,active,status) FROM runs ORDER BY id",
        "SELECT json_array(run_id,phase,slot,generation) FROM run_queue ORDER BY sequence",
        "SELECT json_array(id,state,generation,owner) FROM managed_sessions ORDER BY id",
        "SELECT json_array(run_id,kind,resource_id) FROM run_resources ORDER BY run_id,kind",
    ] {
        let mut q = c.prepare(sql).map_err(storage)?;
        values.push(
            q.query_map([], |r| r.get::<_, String>(0))
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?,
        );
    }
    serde_json::to_string(&values).map_err(storage)
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
        snapshot(&tx)
    }
    /// Compare and freeze serialize with every engine admission/reservation effect.
    pub fn freeze_shutdown(&self, expected: &str) -> Result<Vec<String>, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if snapshot(&tx)? != expected {
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
        let c = Connection::open(&self.path).map_err(storage)?;
        if !frozen(&c)? {
            return Ok(false);
        }
        let pending: bool=c.query_row("SELECT EXISTS(SELECT 1 FROM runs WHERE active!=0 OR NOT json_valid(status) OR COALESCE(json_extract(status,'$.cleanupPending'),1)!=0) OR EXISTS(SELECT 1 FROM run_queue WHERE phase!='settled') OR EXISTS(SELECT 1 FROM managed_sessions WHERE state IN ('leased','acquiring'))",[],|r|r.get(0)).map_err(storage)?;
        Ok(!pending)
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
