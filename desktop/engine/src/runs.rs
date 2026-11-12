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
        let mut s = self.run_status(id)?;
        if matches!(
            s.state,
            RunState::Admitted | RunState::Running | RunState::Held
        ) {
            s.state = RunState::Held;
            s.cleanup_pending = true;
            s.diagnostic="Stop requested. Ownership/secure-cache cleanup must be confirmed; session remains withheld.".into();
            let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
            c.execute(
                "UPDATE runs SET status=?1 WHERE id=?2 AND active=1",
                params![serde_json::to_string(&s).map_err(storage)?, id],
            )
            .map_err(storage)?;
        }
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
            "UPDATE managed_sessions SET state='quarantined' WHERE state='leased'",
            [],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }
}
