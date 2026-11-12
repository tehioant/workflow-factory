//! App-owned serialized managed-cache metadata. Secrets never enter SQLite or DTOs.
use crate::{params, storage, validation, Error, OptionalExtension, Store, TransactionBehavior};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub const CACHE_LIMIT: usize = 65536;
fn cache_publication_guard() -> Result<std::sync::MutexGuard<'static, ()>, Error> {
    static GATE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    GATE.lock().map_err(|_| {
        validation("Managed cache publication uncertain; explicit reconciliation required.")
    })
}
fn refuse_duplicate(
    c: &rusqlite::Connection,
    slot: &str,
    cache: &[u8],
    secure: &mut impl SecureStore,
) -> Result<(), Error> {
    let candidate: serde_json::Value =
        serde_json::from_slice(cache).map_err(|_| validation("Managed cache invalid."))?;
    let mut q = c
        .prepare("SELECT id FROM managed_sessions WHERE id!=?1 AND state!='acquiring' LIMIT 9")
        .map_err(storage)?;
    let slots = q
        .query_map([slot], |r| r.get::<_, String>(0))
        .map_err(storage)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(storage)?;
    if slots.len() > 7 {
        return Err(validation("Managed session bound exceeded."));
    }
    for other in slots {
        let bytes = secure.read(&other)?;
        validate_cache(&bytes)?;
        let retained: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|_| validation("Stored managed cache invalid."))?;
        if candidate["tokens"]["login_owner_id"] == retained["tokens"]["login_owner_id"]
            || candidate["tokens"]["refresh_token"] == retained["tokens"]["refresh_token"]
        {
            return Err(validation("Known duplicate managed login ownership or refresh material; affected operation withheld. Explicit reauthentication required; no slot failover."));
        }
    }
    Ok(())
}

pub fn valid_slot(id: &str) -> Result<(), Error> {
    if id.len() != 36 || uuid::Uuid::parse_str(id).is_err() {
        Err(validation("Invalid managed slot identity."))
    } else {
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lease {
    pub slot: String,
    pub owner: String,
    pub generation: u32,
}
/// Positive current-cache readback and resource removal; never accepted from IPC.
#[derive(Clone, Serialize, Deserialize)]
pub struct CompletionReceipt {
    pub(crate) lease: Lease,
    pub(crate) reusable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSessionStatus {
    pub available: u32,
    pub withheld: u32,
    pub guidance: String,
}
pub trait SecureStore {
    fn read(&mut self, slot: &str) -> Result<Vec<u8>, Error>;
    fn write(&mut self, slot: &str, cache: &[u8]) -> Result<(), Error>;
    /// Success means the exact item is positively absent, including after deletion.
    fn retire(&mut self, _slot: &str) -> Result<(), Error> {
        Err(validation(
            "Secure-store retirement unavailable; slot remains quarantined.",
        ))
    }
}
pub fn validate_cache(cache: &[u8]) -> Result<(), Error> {
    if cache.len() > CACHE_LIMIT {
        return Err(validation("Managed cache exceeds secure storage limit."));
    }
    let v: serde_json::Value = serde_json::from_slice(cache)
        .map_err(|_| validation("Invalid managed cache; sign in again."))?;
    if v["auth_mode"] != "chatgpt" || !v["OPENAI_API_KEY"].is_null() {
        return Err(validation(
            "Only app-acquired ChatGPT managed sessions are supported.",
        ));
    }
    for key in ["access_token", "refresh_token", "id_token"] {
        if v["tokens"][key].as_str().is_none_or(str::is_empty) {
            return Err(validation("Incomplete managed cache; sign in again."));
        }
    }
    if v["tokens"]["login_owner_id"]
        .as_str()
        .is_none_or(str::is_empty)
    {
        return Err(validation("Managed login provenance missing."));
    }
    struct Cache<'a>(&'a [u8]);
    impl crate::auth::CredentialSource for Cache<'_> {
        fn read(&mut self) -> Result<Vec<u8>, crate::auth::CredentialAvailability> {
            Ok(self.0.to_vec())
        }
    }
    if crate::auth::inspect(&mut Cache(cache)) != crate::auth::CredentialAvailability::Available
        || !v["tokens"]["oauth_provenance"].is_object()
    {
        return Err(validation(
            "Managed cache schema/provenance invalid; sign in again.",
        ));
    }
    Ok(())
}
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SignInStatus {
    pub state: String,
    pub url: Option<String>,
    pub code: Option<String>,
    pub message: String,
}
impl Store {
    pub fn retire_quarantined(
        &self,
        secure: &mut impl SecureStore,
        cleanup: impl FnOnce(&str, &[(String, Option<String>)]) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (slot, owner): (String, String) = tx
            .query_row(
                "SELECT id,owner FROM managed_sessions WHERE state='quarantined'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(storage)?;
        let records = {
            let mut q = tx
                .prepare("SELECT kind,resource_id FROM run_resources WHERE run_id=?1")
                .map_err(storage)?;
            let rows = q
                .query_map([&owner], |r| Ok((r.get(0)?, r.get(1)?)))
                .map_err(storage)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(storage)?
        };
        // Explicit irreversible retirement, not writeback or reuse. On any error
        // the quarantined metadata remains and the same operation can be retried.
        cleanup(&owner, &records)?;
        secure.retire(&slot)?;
        tx.execute("DELETE FROM run_resources WHERE run_id=?1", [&owner])
            .map_err(storage)?;
        let body: Option<String> = tx
            .query_row("SELECT status FROM runs WHERE id=?1", [&owner], |r| {
                r.get(0)
            })
            .optional()
            .map_err(storage)?;
        if let Some(body) = body {
            let mut s: crate::runs::RunStatus = serde_json::from_str(&body).map_err(storage)?;
            s.state = crate::runs::RunState::Stopped;
            s.cleanup_pending = false;
            s.diagnostic = "Explicit retirement: owned resources and old secure item confirmed absent. Fresh authorization required.".into();
            tx.execute(
                "UPDATE runs SET active=0,status=?1 WHERE id=?2",
                params![serde_json::to_string(&s).map_err(storage)?, owner],
            )
            .map_err(storage)?;
        }
        tx.execute(
            "DELETE FROM managed_sessions WHERE id=?1 AND state='quarantined'",
            [&slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)
    }
    /// Metadata only, for the native per-operation scope display. No credential read.
    pub fn available_session_scope(&self) -> Result<String, Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let (slot,generation):(String,u32)=c.query_row("SELECT id,generation FROM managed_sessions WHERE state='available' ORDER BY id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|validation("No app-owned managed session is available."))?;
        Ok(format!("Session: {slot} generation {generation}"))
    }
    pub fn begin_acquisition(&self) -> Result<Lease, Error> {
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        crate::lifecycle::admit(&tx)?;
        let count: u32 = tx
            .query_row("SELECT COUNT(*) FROM managed_sessions", [], |r| r.get(0))
            .map_err(storage)?;
        let pending: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE state='acquiring' OR (state='quarantined' AND owner=id))",
                [],
                |r| r.get(0),
            )
            .map_err(storage)?;
        if count >= 8 || pending {
            return Err(validation("At most eight independently acquired app-owned slots and one pending acquisition are supported. No clone, replacement or automatic reauthentication."));
        }
        let slot = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO managed_sessions VALUES(?1,'acquiring',1,?1)",
            [&slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(Lease {
            slot: slot.clone(),
            owner: slot,
            generation: 1,
        })
    }
    pub fn finish_acquisition(
        &self,
        lease: &Lease,
        cache: &[u8],
        secure: &mut impl SecureStore,
        cleanup: impl FnOnce() -> Result<(), Error>,
    ) -> Result<(), Error> {
        let _publication = cache_publication_guard()?;
        validate_cache(cache)?;
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        if c.execute("UPDATE managed_sessions SET state='quarantined' WHERE id=?1 AND owner=?2 AND generation=?3 AND state='acquiring'",params![lease.slot,lease.owner,lease.generation]).map_err(storage)?!=1{return Err(validation("Setup ownership changed."));}
        refuse_duplicate(&c, &lease.slot, cache, secure)?;
        secure.write(&lease.slot, cache)?;
        if secure.read(&lease.slot)? != cache {
            return Err(validation("Native secure-store readback failed."));
        }
        drop(_publication);
        cleanup()?;
        c.execute("UPDATE managed_sessions SET state='available',owner=NULL WHERE id=?1 AND owner=?2 AND generation=?3 AND state='quarantined'",params![lease.slot,lease.owner,lease.generation]).map_err(storage)?;
        Ok(())
    }

    pub fn lease_session(&self, owner: &str) -> Result<Lease, Error> {
        valid_slot(owner)?;
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let (slot,generation):(String,u32)=tx.query_row("SELECT id,generation FROM managed_sessions WHERE state='available' ORDER BY id LIMIT 1",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(|_|validation("No app-owned managed session is available. Sign in explicitly."))?;
        tx.execute(
            "UPDATE managed_sessions SET owner=?1,state='leased' WHERE id=?2 AND state='available'",
            params![owner, slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        Ok(Lease {
            slot,
            owner: owner.into(),
            generation,
        })
    }
    pub fn quarantine_session(&self, lease: &Lease) -> Result<(), Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        c.execute("UPDATE managed_sessions SET state='quarantined' WHERE id=?1 AND owner=?2 AND generation=?3",params![lease.slot,lease.owner,lease.generation]).map_err(storage)?;
        Ok(())
    }

    pub fn managed_session_status(&self) -> Result<ManagedSessionStatus, Error> {
        let c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let available = c
            .query_row(
                "SELECT COUNT(*) FROM managed_sessions WHERE state='available'",
                [],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let withheld = c
            .query_row(
                "SELECT COUNT(*) FROM managed_sessions WHERE state!='available'",
                [],
                |r| r.get(0),
            )
            .map_err(storage)?;
        Ok(ManagedSessionStatus{available,withheld,guidance:"App-owned managed login only; no host credential import or paid fallback. Native secure-store/paging and provider containment qualification required.".into()})
    }
    pub fn checkpoint_session(
        &self,
        lease: &Lease,
        cache: &[u8],
        secure: &mut impl SecureStore,
        cancel: &crate::sandbox::Cancel,
    ) -> Result<Vec<u8>, Error> {
        let (slot, owner, generation) =
            (lease.slot.as_str(), lease.owner.as_str(), lease.generation);
        cancel.check()?;
        let _publication = cache_publication_guard()?;
        validate_cache(cache)?;
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let matches:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE id=?1 AND owner=?2 AND generation=?3 AND state='leased')",params![slot,owner,generation],|r|r.get(0)).map_err(storage)?;
        if !matches {
            return Err(validation("Session lease does not match."));
        }
        // Quarantine commits before any uncertain external persistence/cleanup operation.
        cancel.check()?;
        tx.execute(
            "UPDATE managed_sessions SET state='quarantined' WHERE id=?1",
            [slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        let previous = secure.read(slot)?;
        validate_cache(&previous)?;
        let old: serde_json::Value = serde_json::from_slice(&previous)
            .map_err(|_| validation("Stored managed cache invalid."))?;
        let updated: serde_json::Value = serde_json::from_slice(cache)
            .map_err(|_| validation("Rotated managed cache invalid."))?;
        if old["tokens"]["login_owner_id"] != updated["tokens"]["login_owner_id"]
            || old["tokens"]["account_id"] != updated["tokens"]["account_id"]
            || old["tokens"]["oauth_provenance"] != updated["tokens"]["oauth_provenance"]
        {
            return Err(validation(
                "Managed login identity changed; session quarantined.",
            ));
        }
        cancel.check()?;
        refuse_duplicate(&c, slot, cache, secure)?;
        secure.write(slot, cache)?;
        if secure.read(slot)? != cache {
            return Err(validation(
                "Secure store readback failed; session quarantined.",
            ));
        }
        cancel.check()?;
        if c.execute("UPDATE managed_sessions SET state='leased' WHERE id=?1 AND owner=?2 AND generation=?3 AND state='quarantined'",params![slot,owner,generation]).map_err(storage)? != 1 { return Err(validation("Checkpoint ownership changed.")); }
        Ok(cache.to_vec())
    }
    /// Persist and read back the rotated cache before confirming removal and releasing the lease.
    /// Cleanup callback must positively inspect the recorded resource ID, never a name.
    pub fn finish_session(
        &self,
        slot: &str,
        owner: &str,
        generation: u32,
        cache: &[u8],
        secure: &mut impl SecureStore,
        cleanup: impl FnOnce() -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.finish_owned_session(
            &Lease {
                slot: slot.into(),
                owner: owner.into(),
                generation,
            },
            cache,
            secure,
            true,
            cleanup,
        )
    }
    pub fn finish_owned_session(
        &self,
        lease: &Lease,
        cache: &[u8],
        secure: &mut impl SecureStore,
        reusable: bool,
        cleanup: impl FnOnce() -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.finish_owned_session_receipt(lease, cache, secure, reusable, cleanup)
            .map(|_| ())
    }
    pub fn finish_owned_session_receipt(
        &self,
        lease: &Lease,
        cache: &[u8],
        secure: &mut impl SecureStore,
        reusable: bool,
        cleanup: impl FnOnce() -> Result<(), Error>,
    ) -> Result<CompletionReceipt, Error> {
        let (slot, owner, generation) =
            (lease.slot.as_str(), lease.owner.as_str(), lease.generation);
        let _publication = cache_publication_guard()?;
        validate_cache(cache)?;
        let mut c = rusqlite::Connection::open(&self.path).map_err(storage)?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let matches:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE id=?1 AND owner=?2 AND generation=?3 AND state IN ('leased','quarantined'))",params![slot,owner,generation],|r|r.get(0)).map_err(storage)?;
        if !matches {
            return Err(validation("Session lease does not match."));
        }
        // Quarantine commits before any uncertain external persistence/cleanup operation.
        tx.execute(
            "UPDATE managed_sessions SET state='quarantined' WHERE id=?1",
            [slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        let previous = secure.read(slot)?;
        validate_cache(&previous)?;
        let old: serde_json::Value = serde_json::from_slice(&previous)
            .map_err(|_| validation("Stored managed cache invalid."))?;
        let updated: serde_json::Value = serde_json::from_slice(cache)
            .map_err(|_| validation("Rotated managed cache invalid."))?;
        if old["tokens"]["login_owner_id"] != updated["tokens"]["login_owner_id"]
            || old["tokens"]["account_id"] != updated["tokens"]["account_id"]
            || old["tokens"]["oauth_provenance"] != updated["tokens"]["oauth_provenance"]
        {
            return Err(validation(
                "Managed login identity changed; session quarantined.",
            ));
        }
        refuse_duplicate(&c, slot, cache, secure)?;
        secure.write(slot, cache)?;
        if secure.read(slot)? != cache {
            return Err(validation(
                "Secure store readback failed; session quarantined.",
            ));
        }
        drop(_publication);
        cleanup()?;
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let c = &tx;
        let run_owned: bool = c
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM run_queue WHERE run_id=?1)",
                [owner],
                |r| r.get(0),
            )
            .map_err(storage)?;
        let body: Option<String> = c.query_row("SELECT CASE WHEN finality IS NULL OR length(CAST(finality AS BLOB))<=270336 THEN finality ELSE 'invalid' END FROM run_recovery WHERE run_id=?1 AND slot=?2 AND generation=?3",params![owner,slot,generation],|r|r.get(0)).optional().map_err(storage)?.flatten();
        // Native cleanup has already completed. Missing finality must hold the
        // original run reservation, not publish an unreconstructible release.
        if reusable && run_owned && body.is_none() {
            return Err(validation(
                "Durable chosen finality unavailable; session quarantined.",
            ));
        }
        if reusable && c.execute("UPDATE managed_sessions SET state='available',owner=NULL,generation=generation+1 WHERE id=?1 AND owner=?2 AND generation=?3 AND state='quarantined'",params![slot,owner,generation]).map_err(storage)?!=1{return Err(validation("Session completion ownership changed."));}
        let matches:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM managed_sessions WHERE id=?1 AND ((?4 AND state='available' AND owner IS NULL AND generation=?3+1) OR (NOT ?4 AND state='quarantined' AND owner=?2 AND generation=?3)))",params![slot,owner,generation,reusable],|r|r.get(0)).map_err(storage)?;
        if !matches {
            return Err(validation("Session completion ownership changed."));
        }
        let receipt = CompletionReceipt {
            lease: lease.clone(),
            reusable,
        };
        // Publish completion and session disposition atomically: a crash after
        // release must never lose the storage-only final receipt.
        if let Some(body) = body {
            let mut chosen: crate::runs::settlement::FinalSettlement =
                serde_json::from_str(&body).map_err(|_| validation("Invalid durable finality."))?;
            if chosen.lease.owner != owner
                || chosen.lease.slot != slot
                || chosen.lease.generation != generation
                || chosen.success != reusable
            {
                return Err(validation("Completion disposition changed."));
            }
            chosen.receipt = Some(receipt.clone());
            let body = serde_json::to_string(&chosen).map_err(storage)?;
            if body.len() > 270336 {
                return Err(validation("Final settlement bound exceeded."));
            }
            c.execute(
                "UPDATE run_recovery SET finality=?4 WHERE run_id=?1 AND slot=?2 AND generation=?3",
                params![owner, slot, generation, body],
            )
            .map_err(storage)?;
        }
        tx.commit().map_err(storage)?;
        Ok(receipt)
    }
}
