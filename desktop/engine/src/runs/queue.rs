//! Durable bounded FIFO reservations; no external work under SQLite authority.
use super::{RunState, RunStatus};
use crate::{params, storage, validation, Error, OptionalExtension, Store, TransactionBehavior};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueueSettings {
    pub active: u32,
    pub waiting: u32,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QueueEntry {
    pub id: String,
    pub definition_id: String,
    pub state: RunState,
    pub diagnostic: String,
    pub cleanup_pending: bool,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatus {
    pub settings: QueueSettings,
    pub occupied: u32,
    pub waiting: u32,
    pub entries: Vec<QueueEntry>,
}
pub fn settings(c: &Connection) -> Result<QueueSettings, Error> {
    let value = c
        .query_row(
            "SELECT active,waiting FROM queue_settings WHERE id=1",
            [],
            |r| {
                Ok(QueueSettings {
                    active: r.get(0)?,
                    waiting: r.get(1)?,
                })
            },
        )
        .map_err(storage)?;
    validate(&value)?;
    Ok(value)
}
fn validate(s: &QueueSettings) -> Result<(), Error> {
    if !(1..=8).contains(&s.active) || s.waiting > 64 {
        return Err(validation(
            "Active capacity must be 1..8; waiting capacity must be 0..64.",
        ));
    }
    Ok(())
}
pub(super) fn counts(c: &Connection) -> Result<(u32, u32), Error> {
    let occupied=c.query_row("SELECT COUNT(*) FROM runs r LEFT JOIN run_queue q ON q.run_id=r.id WHERE r.active=1 AND (q.phase='occupied' OR q.run_id IS NULL)",[],|r|r.get(0)).map_err(storage)?;
    let waiting=c.query_row("SELECT COUNT(*) FROM runs r JOIN run_queue q ON q.run_id=r.id WHERE r.active=1 AND q.phase IN ('waiting','withheld')",[],|r|r.get(0)).map_err(storage)?;
    Ok((occupied, waiting))
}
pub(super) fn can_reserve(c: &Connection) -> Result<bool, Error> {
    let (occupied, waiting) = counts(c)?;
    let available:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM managed_sessions s WHERE state='available' AND NOT EXISTS(SELECT 1 FROM run_queue q WHERE q.phase='occupied' AND q.slot=s.id))",[],|r|r.get(0)).map_err(storage)?;
    Ok(waiting == 0 && occupied < settings(c)?.active && available)
}
/// Called only inside the caller's IMMEDIATE transaction. Reserve at most one head.
pub(super) fn reserve(c: &Connection) -> Result<Option<String>, Error> {
    if counts(c)?.0 >= settings(c)?.active {
        return Ok(None);
    }
    let head:Option<(String,String)>=c.query_row("SELECT r.id,r.status FROM run_queue q JOIN runs r ON r.id=q.run_id WHERE q.phase='waiting' AND r.active=1 ORDER BY q.sequence LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
    let Some((id, body)) = head else {
        return Ok(None);
    };
    let mut status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
    if status.id != id || status.state != RunState::Queued {
        return Err(storage("FIFO head identity/state"));
    }
    let slot:Option<(String,u32)>=c.query_row("SELECT id,generation FROM managed_sessions s WHERE state='available' AND NOT EXISTS(SELECT 1 FROM run_queue q WHERE q.phase='occupied' AND q.slot=s.id) ORDER BY id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(storage)?;
    let Some((slot, generation)) = slot else {
        return Ok(None);
    };
    if c.execute("UPDATE managed_sessions SET state='leased',owner=?1 WHERE id=?2 AND generation=?3 AND state='available'",params![id,slot,generation]).map_err(storage)?!=1{return Err(storage("FIFO session reservation"));}
    c.execute("UPDATE run_queue SET phase='occupied',slot=?1,generation=?2 WHERE run_id=?3 AND phase='waiting'",params![slot,generation,id]).map_err(storage)?;
    status.state = RunState::AuthorizationRequired;
    status.diagnostic =
        "Exact reserved session requires single-use native execution confirmation.".into();
    c.execute(
        "UPDATE runs SET status=?1 WHERE id=?2",
        params![serde_json::to_string(&status).map_err(storage)?, id],
    )
    .map_err(storage)?;
    Ok(Some(id))
}
pub fn projection(c: &Connection) -> Result<QueueStatus, Error> {
    let settings = settings(c)?;
    let (occupied, waiting) = counts(c)?;
    // Lazy bounded extraction: never materialize historical prompts/results or oversized JSON.
    let mut q=c.prepare("SELECT r.id,CASE WHEN length(CAST(r.snapshot AS BLOB))<=2097152 THEN CASE WHEN json_valid(r.snapshot) THEN json_extract(r.snapshot,'$.definition.definition.id') END END,CASE WHEN length(CAST(r.status AS BLOB))<=2097152 THEN CASE WHEN json_valid(r.status) THEN json_extract(r.status,'$.state') END END,CASE WHEN length(CAST(r.status AS BLOB))<=2097152 THEN CASE WHEN json_valid(r.status) THEN substr(json_extract(r.status,'$.diagnostic'),1,2048) END END,CASE WHEN length(CAST(r.status AS BLOB))<=2097152 THEN CASE WHEN json_valid(r.status) THEN json_extract(r.status,'$.cleanupPending') END END FROM runs r LEFT JOIN run_queue q ON q.run_id=r.id WHERE r.active=1 ORDER BY COALESCE(q.sequence,0),r.rowid LIMIT 73").map_err(storage)?;
    let rows = q
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, bool>(4)?,
            ))
        })
        .map_err(storage)?;
    let mut entries = Vec::new();
    for row in rows {
        let (id, definition_id, state, diagnostic, cleanup_pending) = row.map_err(storage)?;
        crate::sessions::valid_slot(&id)?;
        crate::sessions::valid_slot(&definition_id)?;
        entries.push(QueueEntry {
            id,
            definition_id,
            state: serde_json::from_value(serde_json::Value::String(state)).map_err(storage)?,
            diagnostic,
            cleanup_pending,
        });
    }
    if entries.len() > 72 {
        return Err(validation(
            "Queue projection exceeds supported capacity; ownership remains withheld.",
        ));
    }
    Ok(QueueStatus {
        settings,
        occupied,
        waiting,
        entries,
    })
}
impl Store {
    pub fn defer_run_authorization(&self, lease: &crate::sessions::Lease) -> Result<(), Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let exact:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM run_queue WHERE run_id=?1 AND slot=?2 AND generation=?3 AND phase='occupied')",params![lease.owner,lease.slot,lease.generation],|r|r.get(0)).map_err(storage)?;
        if exact {
            let body: String = tx
                .query_row("SELECT status FROM runs WHERE id=?1", [&lease.owner], |r| {
                    r.get(0)
                })
                .map_err(storage)?;
            let mut status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
            if status.state == RunState::AuthorizationRequired {
                status.diagnostic="Native confirmation declined; reservation retained. Explicit Authorize or Stop required.".into();
                tx.execute(
                    "UPDATE runs SET status=?1 WHERE id=?2",
                    params![
                        serde_json::to_string(&status).map_err(storage)?,
                        lease.owner
                    ],
                )
                .map_err(storage)?;
            }
        }
        tx.commit().map_err(storage)
    }

    pub fn queue_settings(&self) -> Result<QueueSettings, Error> {
        settings(&Connection::open(&self.path).map_err(storage)?)
    }
    pub fn save_queue_settings(&self, value: QueueSettings) -> Result<QueueSettings, Error> {
        validate(&value)?;
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (occupied, waiting) = counts(&tx)?;
        if value.active < occupied || value.waiting < waiting {
            return Err(validation(
                "Capacity cannot be reduced below occupied or waiting counts.",
            ));
        }
        tx.execute(
            "UPDATE queue_settings SET active=?1,waiting=?2 WHERE id=1",
            params![value.active, value.waiting],
        )
        .map_err(storage)?;
        reserve(&tx)?;
        tx.commit().map_err(storage)?;
        Ok(value)
    }
    pub fn reserve_next_run(&self) -> Result<Option<String>, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let id = reserve(&tx)?;
        tx.commit().map_err(storage)?;
        Ok(id)
    }
    pub fn execution_reservation(&self, id: &str) -> Result<Option<crate::sessions::Lease>, Error> {
        let c = Connection::open(&self.path).map_err(storage)?;
        let row=c.query_row("SELECT q.slot,q.generation FROM run_queue q JOIN runs r ON r.id=q.run_id JOIN managed_sessions s ON s.id=q.slot AND s.generation=q.generation AND s.owner=q.run_id AND s.state='leased' WHERE q.run_id=?1 AND q.phase='occupied' AND r.active=1 AND json_extract(r.status,'$.state')='authorizationRequired'",[id],|r|Ok(crate::sessions::Lease{slot:r.get(0)?,generation:r.get(1)?,owner:id.into()})).optional().map_err(storage)?;
        Ok(row)
    }
    /// Consume native confirmation only for the exact still-current reservation.
    pub fn authorize_reserved_run(&self, lease: &crate::sessions::Lease) -> Result<bool, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let matches:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM run_queue q JOIN managed_sessions s ON s.id=q.slot AND s.generation=q.generation AND s.owner=q.run_id AND s.state='leased' WHERE q.run_id=?1 AND q.slot=?2 AND q.generation=?3 AND q.phase='occupied')",params![lease.owner,lease.slot,lease.generation],|r|r.get(0)).map_err(storage)?;
        if !matches {
            return Ok(false);
        }
        let body: String = tx
            .query_row(
                "SELECT status FROM runs WHERE id=?1 AND active=1",
                [&lease.owner],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let mut s: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        if s.id != lease.owner || s.state != RunState::AuthorizationRequired {
            return Ok(false);
        }
        s.state = RunState::Admitted;
        s.diagnostic.clear();
        tx.execute(
            "UPDATE runs SET status=?1 WHERE id=?2",
            params![serde_json::to_string(&s).map_err(storage)?, lease.owner],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(true)
    }
}
