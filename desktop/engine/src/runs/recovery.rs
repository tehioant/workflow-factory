//! Private durable continuation of the existing resource journal and final settlement.
//! No credentials, execution consent, discovery or current-definition dependencies.
use super::settlement::FinalSettlement;
use crate::{params, storage, validation, Error, OptionalExtension, Store};
use crate::{sandbox::Resources, sessions::Lease};

impl Store {
    pub fn retain_recovery_resources(
        &self,
        lease: &Lease,
        resources: Option<&Resources>,
    ) -> Result<(), Error> {
        self.cleanup_records(lease)?;
        if let Some(r) = resources {
            if r.owner != lease.owner
                || r.peer.is_some()
                || r.started_at
                    .as_ref()
                    .is_none_or(|e| e.is_empty() || e.len() > 128)
            {
                return Err(validation(
                    "Recovery resource identity or epoch unavailable.",
                ));
            }
        }
        let body = resources
            .map(serde_json::to_string)
            .transpose()
            .map_err(storage)?;
        if body.as_ref().is_some_and(|b| b.len() > 8192) {
            return Err(validation("Recovery metadata bound exceeded."));
        }
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        c.execute(
            "INSERT OR IGNORE INTO run_recovery(run_id,slot,generation) VALUES(?1,?2,?3)",
            params![lease.owner, lease.slot, lease.generation],
        )
        .map_err(storage)?;
        if c.execute("UPDATE run_recovery SET resources=?4 WHERE run_id=?1 AND slot=?2 AND generation=?3 AND finality IS NULL",params![lease.owner,lease.slot,lease.generation,body]).map_err(storage)? != 1 {
            return Err(validation("Recovery authority changed; ownership held."));
        }
        Ok(())
    }

    pub fn interrupted_recovery(&self, selected: &str) -> Result<(Lease, Resources), Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let (slot,generation,body): (String,u32,String) = c.query_row("SELECT slot,generation,resources FROM run_recovery WHERE run_id=?1 AND length(CAST(resources AS BLOB))<=8192",[selected],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(storage)?;
        let lease = Lease {
            slot,
            owner: selected.into(),
            generation,
        };
        self.cleanup_records(&lease)?;
        let r: Resources = serde_json::from_str(&body)
            .map_err(|_| validation("Recovery metadata unavailable."))?;
        if r.owner != selected
            || r.peer.is_some()
            || r.started_at
                .as_ref()
                .is_none_or(|e| e.is_empty() || e.len() > 128)
        {
            return Err(validation("Recovery authority uncertain; ownership held."));
        }
        Ok((lease, r))
    }

    pub fn retain_final_settlement(&self, chosen: &FinalSettlement) -> Result<(), Error> {
        if chosen.receipt.as_ref().is_some_and(|r| {
            r.lease.owner != chosen.lease.owner
                || r.lease.slot != chosen.lease.slot
                || r.lease.generation != chosen.lease.generation
                || r.reusable != chosen.success
        }) {
            return Err(validation("Final receipt identity or disposition changed."));
        }
        let body = serde_json::to_string(chosen).map_err(storage)?;
        if body.len() > 270336 {
            return Err(validation("Final settlement bound exceeded."));
        }
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let previous: Option<String> = c
            .query_row(
                "SELECT CASE WHEN finality IS NULL OR length(CAST(finality AS BLOB))<=270336 THEN finality ELSE 'invalid' END FROM run_recovery WHERE run_id=?1 AND slot=?2 AND generation=?3",
                params![
                    chosen.lease.owner,
                    chosen.lease.slot,
                    chosen.lease.generation
                ],
                |r| r.get(0),
            )
            .optional()
            .map_err(storage)?
            .flatten();
        if let Some(previous) = previous {
            let old: FinalSettlement = serde_json::from_str(&previous).map_err(storage)?;
            if old.text != chosen.text
                || old.diagnostic != chosen.diagnostic
                || old.success != chosen.success
                || (old.receipt.is_some() && chosen.receipt.is_none())
            {
                return Err(validation("Chosen final settlement is immutable."));
            }
        }
        c.execute("INSERT OR IGNORE INTO run_recovery(run_id,slot,generation) SELECT run_id,slot,generation FROM run_queue WHERE run_id=?1 AND slot=?2 AND generation=?3 AND phase IN ('occupied','settled')",params![chosen.lease.owner,chosen.lease.slot,chosen.lease.generation]).map_err(storage)?;
        if c.execute(
            "UPDATE run_recovery SET finality=?4 WHERE run_id=?1 AND slot=?2 AND generation=?3",
            params![
                chosen.lease.owner,
                chosen.lease.slot,
                chosen.lease.generation,
                body
            ],
        )
        .map_err(storage)?
            != 1
        {
            return Err(validation("Durable final settlement unavailable."));
        }
        Ok(())
    }

    /// Only a receipt persisted after actual cleanup grants storage-only replay.
    pub fn retained_final_settlement(
        &self,
        selected: &str,
    ) -> Result<Option<FinalSettlement>, Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let row: Option<(String,u32,String)> = c.query_row("SELECT slot,generation,finality FROM run_recovery WHERE run_id=?1 AND finality IS NOT NULL AND length(CAST(finality AS BLOB))<=270336",[selected],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(storage)?;
        let Some((slot, generation, body)) = row else {
            return Ok(None);
        };
        let chosen: FinalSettlement = serde_json::from_str(&body)
            .map_err(|_| validation("Durable final settlement invalid."))?;
        if chosen.lease.owner != selected
            || chosen.lease.slot != slot
            || chosen.lease.generation != generation
        {
            return Err(validation("Durable settlement identity changed."));
        }
        Ok(Some(chosen))
    }
}
