//! Durable one-block admission. External effects never occur inside a transaction.
use crate::{
    params, read_configuration, resolve_inputs, storage, stored_definition, validation, Error,
    OptionalExtension, Store, TransactionBehavior,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StartRun {
    pub request_id: String,
    pub definition_id: String,
    pub expected_revision: u32,
    pub expected_configuration_revision: u32,
    pub supplied: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum RunState {
    Admitted,
    Running,
    Succeeded,
    Failed,
    Stopped,
    Blocked,
    Held,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct RunStatus {
    pub id: String,
    pub request_id: String,
    pub state: RunState,
    pub final_response: Option<String>,
    pub diagnostic: String,
    pub cleanup_pending: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub definition: crate::SavedDefinition,
    pub configuration: crate::SavedConfiguration,
    pub inputs: crate::PreparedInputs,
    pub image_id: String,
}
/// Backend-only observations, never deserialized from IPC.
pub struct Admission {
    pub image_id: String,
    pub blocker: Option<String>,
}
impl Store {
    /// Retain validated, redacted output without declaring cleanup or completion.
    pub fn retain_run_output(&self, id: &str, text: &str) -> Result<(), Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let body: String = tx
            .query_row(
                "SELECT status FROM runs WHERE id=?1 AND active=1",
                [id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let mut status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        status.final_response = Some(text.into());
        tx.execute(
            "UPDATE runs SET status=?1 WHERE id=?2 AND active=1",
            params![serde_json::to_string(&status).map_err(storage)?, id],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }
    pub fn claim_run(&self, id: &str) -> Result<Option<(Snapshot, crate::sessions::Lease)>, Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let row: Option<(String, String)> = tx
            .query_row(
                "SELECT status,snapshot FROM runs WHERE id=?1 AND active=1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let Some((body, snapshot)) = row else {
            return Ok(None);
        };
        let mut status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        if status.state != RunState::Admitted {
            return Ok(None);
        }
        let (slot, generation): (String, u32) = tx
            .query_row(
                "SELECT id,generation FROM managed_sessions WHERE owner=?1 AND state='leased'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        status.state = RunState::Running;
        tx.execute(
            "UPDATE runs SET status=?1 WHERE id=?2",
            params![serde_json::to_string(&status).map_err(storage)?, id],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(Some((
            serde_json::from_str(&snapshot).map_err(storage)?,
            crate::sessions::Lease {
                slot,
                owner: id.into(),
                generation,
            },
        )))
    }
    pub fn resource_intent(
        &self,
        id: &str,
        kind: &str,
        resource: Option<&str>,
    ) -> Result<(), Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        if let Some(resource) = resource {
            if c.execute("UPDATE run_resources SET resource_id=?1 WHERE run_id=?2 AND kind=?3 AND resource_id IS NULL",params![resource,id,kind]).map_err(storage)?!=1{return Err(storage("ownership receipt"));}
        } else {
            c.execute(
                "INSERT INTO run_resources VALUES(?1,?2,NULL)",
                params![id, kind],
            )
            .map_err(storage)?;
        }
        Ok(())
    }
    pub fn finish_run(
        &self,
        id: &str,
        final_response: Option<String>,
        diagnostic: &str,
        clean: bool,
        success: bool,
    ) -> Result<(), Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let body: String = tx
            .query_row(
                "SELECT status FROM runs WHERE id=?1 AND active=1",
                [id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let mut status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        status.state = if !clean {
            RunState::Held
        } else if status.state == RunState::Held {
            RunState::Stopped
        } else if success {
            RunState::Succeeded
        } else {
            RunState::Failed
        };
        status.final_response = final_response.or(status.final_response);
        status.diagnostic = diagnostic.into();
        status.cleanup_pending = !clean;
        tx.execute(
            "UPDATE runs SET status=?1,active=?2 WHERE id=?3",
            params![serde_json::to_string(&status).map_err(storage)?, !clean, id],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }

    /// Resolve a retry before native preflight/confirmation; never read a new
    /// session or mutable configuration to report an existing request outcome.
    pub fn recorded_run(&self, request: &StartRun) -> Result<Option<RunStatus>, Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let row: Option<(String, String)> = c
            .query_row(
                "SELECT request,status FROM runs WHERE request_id=?1",
                [&request.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        row.map(|(original, status)| {
            if original != serde_json::to_string(request).map_err(storage)? {
                return Err(validation(
                    "Request identity already binds different inputs or revisions.",
                ));
            }
            serde_json::from_str(&status).map_err(storage)
        })
        .transpose()
    }
    pub fn admit_run(&self, request: StartRun, admission: Admission) -> Result<RunStatus, Error> {
        if request.request_id.len() != 36 || uuid::Uuid::parse_str(&request.request_id).is_err() {
            return Err(validation("Request identity must be a UUID."));
        }
        let payload = serde_json::to_string(&request).map_err(storage)?;
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT request,status FROM runs WHERE request_id=?1",
                [&request.request_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        if let Some((original, status)) = existing {
            if original != payload {
                return Err(validation(
                    "Request identity already binds different inputs or revisions.",
                ));
            }
            return serde_json::from_str(&status).map_err(storage);
        }
        if tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM runs WHERE active=1)",
                [],
                |r| r.get::<_, bool>(0),
            )
            .map_err(storage)?
        {
            return Err(Error::Busy {
                message: "An active run or unresolved ownership hold exists.".into(),
            });
        }
        let row: Option<(u32, String)> = tx
            .query_row(
                "SELECT revision,body FROM revisions WHERE id=?1 ORDER BY revision DESC LIMIT 1",
                [&request.definition_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let (revision, body) =
            row.ok_or_else(|| validation("Save the workflow before starting."))?;
        if revision != request.expected_revision {
            return Err(validation(
                "Saved workflow changed. Reload before starting.",
            ));
        }
        let definition = stored_definition(&request.definition_id, revision, &body)?;
        if definition.definition.blocks.len() != 1 || !definition.definition.connections.is_empty()
        {
            return Err(validation("This version runs exactly one AI block."));
        }
        let config_id = definition.definition.blocks[0]
            .configuration_id
            .as_deref()
            .ok_or_else(|| validation("Select a Codex configuration."))?;
        let configuration = read_configuration(&tx, config_id)?
            .ok_or_else(|| validation("Configuration missing."))?;
        if configuration.revision != request.expected_configuration_revision {
            return Err(validation(
                "Configuration changed. Refresh before starting.",
            ));
        }
        let inputs = resolve_inputs(&definition.definition, request.supplied.clone())?;
        let snapshot = Snapshot {
            definition,
            configuration,
            inputs,
            image_id: admission.image_id,
        };
        let blocked = admission.blocker.is_some();
        let status = RunStatus {
            id: uuid::Uuid::new_v4().to_string(),
            request_id: request.request_id,
            state: if blocked {
                RunState::Blocked
            } else {
                RunState::Admitted
            },
            final_response: None,
            diagnostic: admission.blocker.unwrap_or_default(),
            cleanup_pending: false,
        };
        if !blocked {
            let slot: String = tx
                .query_row(
                    "SELECT id FROM managed_sessions WHERE state='available' ORDER BY id LIMIT 1",
                    [],
                    |r| r.get(0),
                )
                .map_err(|_| validation("No app-owned managed session is available."))?;
            tx.execute("UPDATE managed_sessions SET state='leased',owner=?1 WHERE id=?2 AND state='available'",params![status.id,slot]).map_err(storage)?;
        }
        tx.execute("INSERT INTO runs(id,request_id,request,snapshot,status,active) VALUES(?1,?2,?3,?4,?5,?6)",params![status.id,status.request_id,payload,serde_json::to_string(&snapshot).map_err(storage)?,serde_json::to_string(&status).map_err(storage)?,!blocked]).map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(status)
    }
    pub fn run_status(&self, id: &str) -> Result<RunStatus, Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let body: String = c
            .query_row("SELECT status FROM runs WHERE id=?1", [id], |r| r.get(0))
            .map_err(storage)?;
        let status: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        if status.id != id {
            return Err(storage("run identity"));
        }
        Ok(status)
    }
    pub fn latest_run(&self) -> Result<Option<RunStatus>, Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let body: Option<String> = c
            .query_row(
                "SELECT status FROM runs ORDER BY rowid DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?;
        body.map(|s| serde_json::from_str(&s).map_err(storage))
            .transpose()
    }
    pub fn stop_run(&self, id: &str) -> Result<RunStatus, Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let body: String = tx
            .query_row("SELECT status FROM runs WHERE id=?1", [id], |r| r.get(0))
            .map_err(storage)?;
        let mut s: RunStatus = serde_json::from_str(&body).map_err(storage)?;
        if s.id != id {
            return Err(storage("run identity"));
        }
        let intent: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM run_resources WHERE run_id=?1)",
                [id],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if s.state == RunState::Admitted && !intent {
            // The same IMMEDIATE transaction excludes claim and intent races. Only
            // this unclaimed reservation is safe to release; never a running lease.
            let released = tx.execute("UPDATE managed_sessions SET state='available',owner=NULL WHERE owner=?1 AND state='leased'", [id]).map_err(storage)?;
            if released != 1 {
                return Err(storage("Unclaimed reservation ownership changed"));
            }
            s.state = RunState::Stopped;
            s.cleanup_pending = false;
            s.diagnostic =
                "Stopped before worker claim; no resource intent exists. Reservation released."
                    .into();
            tx.execute(
                "UPDATE runs SET status=?1,active=0 WHERE id=?2 AND active=1",
                params![serde_json::to_string(&s).map_err(storage)?, id],
            )
            .map_err(storage)?;
        } else if matches!(
            s.state,
            RunState::Admitted | RunState::Running | RunState::Held
        ) {
            s.state = RunState::Held;
            s.cleanup_pending = true;
            s.diagnostic="Stop requested. Ownership/secure-cache cleanup must be confirmed; session remains withheld.".into();
            tx.execute(
                "UPDATE runs SET status=?1 WHERE id=?2 AND active=1",
                params![serde_json::to_string(&s).map_err(storage)?, id],
            )
            .map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        Ok(s)
    }
    pub fn withhold_unfinished_runs(&self) -> Result<(), Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let bodies: Vec<String> = {
            let mut q = tx
                .prepare("SELECT status FROM runs WHERE active=1")
                .map_err(storage)?;
            let rows = q.query_map([], |r| r.get(0)).map_err(storage)?;
            rows.collect::<Result<_, _>>().map_err(storage)?
        };
        for body in bodies {
            let mut s: RunStatus = serde_json::from_str(&body).map_err(storage)?;
            s.state = RunState::Held;
            s.cleanup_pending = true;
            s.diagnostic="Interrupted run withheld. No automatic resume, resource deletion or session reuse.".into();
            tx.execute(
                "UPDATE runs SET status=?1 WHERE id=?2",
                params![serde_json::to_string(&s).map_err(storage)?, s.id],
            )
            .map_err(storage)?;
        }
        tx.execute(
            "UPDATE managed_sessions SET state='quarantined' WHERE state IN ('leased','acquiring')",
            [],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }
}
