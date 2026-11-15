//! Direct-model reservation and storage authority; no credentials or network.
use super::{ExecutionKind, RunState, RunStatus, Snapshot};
use crate::{params, storage, validation, Error, OptionalExtension, Store, TransactionBehavior};
use rusqlite::Connection;
const MAX_DIAGNOSTIC_BYTES: usize = 2048;
// JSON can encode one decoded byte as six bytes (\u00XX). UUID, i64,
// field names, punctuation and booleans fit in the fixed envelope.
const MAX_FINALITY_BYTES: usize =
    6 * (crate::providers::MAX_OUTPUT_BYTES + MAX_DIAGNOSTIC_BYTES) + 256;
// At most 100 blocks: each resolved prompt is bounded to 1 MiB, each
// output to 256 KiB, plus the duplicated final output and metadata envelope.
pub(super) const MAX_STATUS_BYTES: usize = 6
    * (100 * (1_048_576 + crate::providers::MAX_OUTPUT_BYTES) + crate::providers::MAX_OUTPUT_BYTES)
    + 2 * 1024 * 1024;
fn validate_finality(f: &DirectSettlement) -> Result<(), Error> {
    if f.text
        .as_ref()
        .is_some_and(|s| s.len() > crate::providers::MAX_OUTPUT_BYTES)
        || f.diagnostic.len() > MAX_DIAGNOSTIC_BYTES
        || f.reservation.run_id.len() != 36
        || uuid::Uuid::parse_str(&f.reservation.run_id).is_err()
        || f.reservation.sequence <= 0
    {
        return Err(validation(
            "Direct finality exceeds bounds or has invalid identity.",
        ));
    }
    Ok(())
}
fn retained(c: &Connection, id: &str) -> Result<Option<(String, DirectSettlement)>, Error> {
    let row: Option<(i64, Option<String>)> = c.query_row(
        "SELECT sequence,CASE WHEN length(CAST(body AS BLOB))<=?2 THEN body END FROM direct_run_settlements WHERE run_id=?1",
        params![id, MAX_FINALITY_BYTES as i64], |r| Ok((r.get(0)?, r.get(1)?)),
    ).optional().map_err(storage)?;
    row.map(|(sequence, body)| {
        let body = body.ok_or_else(|| validation("Retained direct finality exceeds bounds."))?;
        let f: DirectSettlement = serde_json::from_str(&body).map_err(storage)?;
        validate_finality(&f)?;
        if f.reservation.run_id != id || f.reservation.sequence != sequence {
            return Err(validation("Direct finality identity changed."));
        }
        Ok((body, f))
    })
    .transpose()
}
pub(super) const RESTART_HOLD:&str="Interrupted direct request withheld; never resent. Remote processing and API charges may already have occurred. Explicit Stop or retained-output storage recovery only.";
pub(super) const STOP_STORAGE_HOLD:&str="Direct Stop with no local HTTP worker; final storage pending. No execution retry or remote cancellation claim.";
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectReservation {
    pub run_id: String,
    pub sequence: i64,
}
#[derive(Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectSettlement {
    pub reservation: DirectReservation,
    pub text: Option<String>,
    pub diagnostic: String,
    pub success: bool,
}
#[derive(Clone)]
pub enum ExecutionReservation {
    Codex(crate::sessions::Lease),
    Direct(DirectReservation),
}
impl From<&crate::sessions::Lease> for ExecutionReservation {
    fn from(r: &crate::sessions::Lease) -> Self {
        Self::Codex(r.clone())
    }
}
impl From<&DirectReservation> for ExecutionReservation {
    fn from(r: &DirectReservation) -> Self {
        Self::Direct(r.clone())
    }
}
impl From<&ExecutionReservation> for ExecutionReservation {
    fn from(r: &ExecutionReservation) -> Self {
        r.clone()
    }
}
pub(super) fn checked(
    c: &Connection,
    r: &DirectReservation,
) -> Result<(Snapshot, RunStatus), Error> {
    let (snapshot,status):(String,String)=c.query_row("SELECT r.snapshot,r.status FROM runs r JOIN run_queue q ON q.run_id=r.id WHERE r.id=?1 AND q.sequence=?2 AND q.execution_kind='directModel' AND q.phase='occupied' AND q.slot IS NULL AND q.generation IS NULL AND r.active=1 AND length(CAST(r.snapshot AS BLOB))<=2097152 AND length(CAST(r.status AS BLOB))<=?3 AND NOT EXISTS(SELECT 1 FROM run_resources WHERE run_id=r.id)",params![r.run_id,r.sequence,MAX_STATUS_BYTES as i64],|row|Ok((row.get(0)?,row.get(1)?))).map_err(storage)?;
    let snapshot: Snapshot = serde_json::from_str(&snapshot).map_err(storage)?;
    let status: RunStatus = serde_json::from_str(&status).map_err(storage)?;
    snapshot.validate_kind()?;
    if status.id != r.run_id || snapshot.execution_kind != ExecutionKind::DirectModel {
        return Err(validation("Direct reservation identity changed."));
    }
    // Reuse all immutable history identity/revision/prompt checks, not only a tag.
    let detail = super::history::detail(c, &snapshot.definition.definition.id, &r.run_id)?;
    if detail.execution_kind != ExecutionKind::DirectModel {
        return Err(validation("Direct capture changed."));
    }
    Ok((snapshot, status))
}
impl Store {
    pub fn request_execution_kind(
        &self,
        request: &super::StartRun,
    ) -> Result<ExecutionKind, Error> {
        let c = Connection::open(&self.path).map_err(storage)?;
        let tx = c.unchecked_transaction().map_err(storage)?;
        let (revision, body): (u32, String) = tx
            .query_row(
                "SELECT revision,body FROM revisions WHERE id=?1 ORDER BY revision DESC LIMIT 1",
                [&request.definition_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        if revision != request.expected_revision {
            return Err(validation(
                "Saved workflow changed. Reload before starting.",
            ));
        }
        let d = crate::stored_definition(&request.definition_id, revision, &body)?;
        let mut kind = None;
        let order = crate::prompts::ordered_ids(&d.definition)?;
        if order.is_empty() {
            return Err(validation("Add an AI block before starting."));
        }
        for id in &order {
            let b = d
                .definition
                .blocks
                .iter()
                .find(|b| &b.id == id)
                .ok_or_else(|| validation("Block missing."))?;
            let config_id = b
                .configuration_id
                .as_deref()
                .ok_or_else(|| validation("Select a configuration for every block."))?;
            let config = crate::read_configuration(&tx, config_id)?
                .ok_or_else(|| validation("Configuration missing."))?;
            let expected = request
                .expected_configuration_revisions
                .get(config_id)
                .copied()
                .or_else(|| {
                    (order.len() == 1 && request.expected_configuration_revisions.is_empty())
                        .then_some(request.expected_configuration_revision)
                });
            if expected != Some(config.revision) {
                return Err(validation(
                    "Configuration changed. Refresh before starting.",
                ));
            }
            let current = if matches!(config.config, crate::Configuration::DirectModel(_)) {
                ExecutionKind::DirectModel
            } else {
                ExecutionKind::Codex
            };
            if kind.is_some_and(|k| k != current) {
                return Err(validation("Mixed direct/agent runs are unsupported. Use an all-direct or all-Codex chain."));
            }
            kind = Some(current);
        }
        kind.ok_or_else(|| validation("Empty run."))
    }
    pub fn retain_direct_settlement(&self, f: &DirectSettlement) -> Result<(), Error> {
        validate_finality(f)?;
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let body = serde_json::to_string(f).map_err(storage)?;
        if body.len() > MAX_FINALITY_BYTES {
            return Err(validation("Direct finality exceeds bounds."));
        }
        if let Some((old, _)) = retained(&tx, &f.reservation.run_id)? {
            if old == body {
                return Ok(());
            }
            return Err(validation("Chosen direct finality is immutable."));
        }
        checked(&tx, &f.reservation)?;
        tx.execute(
            "INSERT INTO direct_run_settlements VALUES(?1,?2,?3)",
            params![f.reservation.run_id, f.reservation.sequence, body],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }
    pub fn settle_direct(&self, f: &DirectSettlement) -> Result<(), Error> {
        validate_finality(f)?;
        let r = &f.reservation;
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let exact:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM direct_run_settlements WHERE run_id=?1 AND sequence=?2 AND body=?3)",params![r.run_id,r.sequence,serde_json::to_string(f).map_err(storage)?],|r|r.get(0)).map_err(storage)?;
        if !exact {
            return Err(validation("Durable direct finality missing or changed."));
        }
        let (body,active,phase,sequence,kind,slot,generation):(String,bool,String,i64,String,Option<String>,Option<u32>)=tx.query_row("SELECT r.status,r.active,q.phase,q.sequence,q.execution_kind,q.slot,q.generation FROM runs r JOIN run_queue q ON q.run_id=r.id WHERE r.id=?1 AND length(CAST(r.status AS BLOB))<=?2",params![r.run_id,MAX_STATUS_BYTES as i64],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?))).map_err(storage)?;
        let mut s: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        if s.id != r.run_id
            || sequence != r.sequence
            || kind != "directModel"
            || slot.is_some()
            || generation.is_some()
        {
            return Err(validation("Direct settlement authority changed."));
        }
        if !active {
            return if phase == "settled"
                && !s.cleanup_pending
                && s.final_response == f.text
                && s.diagnostic == f.diagnostic
                && (s.state == RunState::Stopped
                    || s.state
                        == if f.success {
                            RunState::Succeeded
                        } else {
                            RunState::Failed
                        })
            {
                Ok(())
            } else {
                Err(validation("Terminal direct result is immutable."))
            };
        }
        let (snapshot, _) = checked(&tx, r)?;
        if f.success
            && (s.blocks.len() != snapshot.plans.len()
                || s.blocks
                    .iter()
                    .any(|b| b.state != RunState::Succeeded || b.final_response.is_none())
                || s.blocks.last().and_then(|b| b.final_response.as_ref()) != f.text.as_ref())
        {
            return Err(validation(
                "Direct success requires all durable exact outputs.",
            ));
        }
        s.state = if s.state == RunState::Cancelling {
            RunState::Stopped
        } else if f.success {
            RunState::Succeeded
        } else {
            RunState::Failed
        };
        s.final_response = f.text.clone();
        s.diagnostic = f.diagnostic.clone();
        s.cleanup_pending = false;
        tx.execute(
            "UPDATE runs SET status=?1,active=0 WHERE id=?2",
            params![serde_json::to_string(&s).map_err(storage)?, r.run_id],
        )
        .map_err(storage)?;
        tx.execute("UPDATE run_queue SET phase='settled' WHERE run_id=?1 AND sequence=?2 AND phase='occupied'",params![r.run_id,r.sequence]).map_err(storage)?;
        tx.commit().map_err(storage)
    }
    pub fn retained_direct_settlement(&self, id: &str) -> Result<Option<DirectSettlement>, Error> {
        let c = Connection::open(&self.path).map_err(storage)?;
        retained(&c, id).map(|row| row.map(|(_, f)| f))
    }
    pub fn hold_direct_persistence(
        &self,
        f: &DirectSettlement,
        diagnostic: &str,
    ) -> Result<(), Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (_, mut s) = checked(&tx, &f.reservation)?;
        if s.state != RunState::Cancelling {
            s.state = RunState::Held;
        }
        s.cleanup_pending = false;
        s.diagnostic = diagnostic.into();
        tx.execute(
            "UPDATE runs SET status=?1 WHERE id=?2",
            params![
                serde_json::to_string(&s).map_err(storage)?,
                f.reservation.run_id
            ],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }
    pub fn direct_reservation(&self, id: &str) -> Result<Option<DirectReservation>, Error> {
        let c = Connection::open(&self.path).map_err(storage)?;
        let sequence:Option<i64>=c.query_row("SELECT sequence FROM run_queue WHERE run_id=?1 AND phase='occupied' AND execution_kind='directModel'",[id],|r|r.get(0)).optional().map_err(storage)?;
        sequence
            .map(|sequence| {
                let r = DirectReservation {
                    run_id: id.into(),
                    sequence,
                };
                checked(&c, &r)?;
                Ok(r)
            })
            .transpose()
    }
    pub fn authorize_direct(&self, r: &DirectReservation) -> Result<bool, Error> {
        self.direct_transition(r, RunState::AuthorizationRequired, RunState::Admitted)
            .map(|s| s.is_some())
    }
    pub fn claim_direct(&self, r: &DirectReservation) -> Result<Option<Snapshot>, Error> {
        self.direct_transition(r, RunState::Admitted, RunState::Running)
    }
    fn direct_transition(
        &self,
        r: &DirectReservation,
        from: RunState,
        to: RunState,
    ) -> Result<Option<Snapshot>, Error> {
        let mut c = Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        if crate::lifecycle::frozen(&tx)? {
            return Ok(None);
        }
        let Ok((snapshot, mut status)) = checked(&tx, r) else {
            return Ok(None);
        };
        if status.state != from || status.cleanup_pending {
            return Ok(None);
        }
        status.state = to;
        status.diagnostic.clear();
        tx.execute(
            "UPDATE runs SET status=?1 WHERE id=?2",
            params![serde_json::to_string(&status).map_err(storage)?, r.run_id],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(Some(snapshot))
    }
    pub fn check_direct_send(&self, r: &DirectReservation) -> Result<(), Error> {
        let c = Connection::open(&self.path).map_err(storage)?;
        let tx = c.unchecked_transaction().map_err(storage)?;
        crate::lifecycle::admit(&tx)?;
        let (_, s) = checked(&tx, r)?;
        if s.state != RunState::Running || s.cleanup_pending {
            return Err(validation("Direct send stopped or held."));
        }
        Ok(())
    }
    pub fn worker_reservation(&self, id: &str) -> Result<Option<ExecutionReservation>, Error> {
        if let Some(r) = self.direct_reservation(id)? {
            return Ok(Some(ExecutionReservation::Direct(r)));
        }
        self.execution_reservation(id)
            .map(|r| r.map(ExecutionReservation::Codex))
    }
    pub fn authorize_execution(&self, r: &ExecutionReservation) -> Result<bool, Error> {
        match r {
            ExecutionReservation::Codex(r) => self.authorize_reserved_run(r),
            ExecutionReservation::Direct(r) => self.authorize_direct(r),
        }
    }
    pub fn defer_execution(&self, r: &ExecutionReservation) -> Result<(), Error> {
        match r {
            ExecutionReservation::Codex(r) => self.defer_run_authorization(r),
            ExecutionReservation::Direct(r) => {
                let mut c = Connection::open(&self.path).map_err(storage)?;
                let tx = c
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(storage)?;
                let (_, mut s) = checked(&tx, r)?;
                if s.state == RunState::AuthorizationRequired {
                    s.diagnostic =
                        "Native confirmation declined; explicit Authorize or Stop required.".into();
                    tx.execute(
                        "UPDATE runs SET status=?1 WHERE id=?2",
                        params![serde_json::to_string(&s).map_err(storage)?, r.run_id],
                    )
                    .map_err(storage)?;
                }
                tx.commit().map_err(storage)
            }
        }
    }
}
