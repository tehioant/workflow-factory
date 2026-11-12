//! Storage-only final replay. Receipts contain no credentials or external authority.
use super::{RunState, RunStatus};
use crate::sessions::{CompletionReceipt, Lease};
use crate::{params, storage, validation, Error, Store, TransactionBehavior};
#[derive(Clone)]
pub struct FinalSettlement {
    pub lease: Lease,
    pub text: Option<String>,
    pub diagnostic: String,
    pub success: bool,
    pub receipt: Option<CompletionReceipt>,
}
impl Store {
    pub fn settle_run(&self, finality: &FinalSettlement) -> Result<(), Error> {
        let lease = &finality.lease;
        let receipt = finality.receipt.as_ref().ok_or_else(|| {
            validation("Final cache/cleanup receipt missing; ownership remains held.")
        })?;
        if receipt.lease.slot != lease.slot
            || receipt.lease.owner != lease.owner
            || receipt.lease.generation != lease.generation
            || finality.success != receipt.reusable
        {
            return Err(validation("Final receipt identity or disposition changed."));
        }
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (body, active): (String, bool) = tx
            .query_row(
                "SELECT status,active FROM runs WHERE id=?1",
                [&lease.owner],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        let mut status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        let (phase, slot, generation): (String, String, u32) = tx
            .query_row(
                "SELECT phase,slot,generation FROM run_queue WHERE run_id=?1",
                [&lease.owner],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(storage)?;
        if status.id != lease.owner || slot != lease.slot || generation != lease.generation {
            return Err(validation("Final reservation identity changed."));
        }
        if !active {
            // Exact lost-ack replay is read-only, even if the released session advanced.
            let outcome = if status.state == RunState::Stopped {
                RunState::Stopped
            } else if finality.success {
                RunState::Succeeded
            } else {
                RunState::Failed
            };
            if phase == "settled"
                && status.state == outcome
                && status.final_response == finality.text
                && status.diagnostic == finality.diagnostic
                && !status.cleanup_pending
            {
                return Ok(());
            }
            return Err(validation("Terminal result is immutable."));
        }
        if phase != "occupied" {
            return Err(validation("Only exact occupied reservation may settle."));
        }
        let matches:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE id=?1 AND ((?4 AND state='available' AND owner IS NULL AND generation=?3+1) OR (NOT ?4 AND state='quarantined' AND owner=?2 AND generation=?3)))",params![lease.slot,lease.owner,lease.generation,receipt.reusable],|r|r.get(0)).map_err(storage)?;
        if !matches {
            return Err(validation("Current session completion receipt changed."));
        }
        if finality.success
            && status
                .blocks
                .iter()
                .any(|b| b.state != RunState::Succeeded || b.final_response.is_none())
        {
            return Err(validation("Incomplete block output cannot succeed."));
        }
        status.state = if status.state == RunState::Cancelling {
            RunState::Stopped
        } else if finality.success {
            RunState::Succeeded
        } else {
            RunState::Failed
        };
        status.final_response = finality.text.clone();
        status.diagnostic = finality.diagnostic.clone();
        status.cleanup_pending = false;
        tx.execute(
            "UPDATE runs SET status=?1,active=0 WHERE id=?2 AND active=1",
            params![
                serde_json::to_string(&status).map_err(storage)?,
                lease.owner
            ],
        )
        .map_err(storage)?;
        tx.execute(
            "UPDATE run_queue SET phase='settled' WHERE run_id=?1 AND phase='occupied'",
            [&lease.owner],
        )
        .map_err(storage)?;
        // This action never reserves or dispatches another run.
        tx.commit().map_err(storage)
    }
}
