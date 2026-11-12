//! App-owned serialized managed-cache metadata. Secrets never enter SQLite or DTOs.
use crate::{params, storage, validation, Error, Store, TransactionBehavior};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub const CACHE_LIMIT: usize = 65536;
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
    if v["login_owner_id"].as_str().is_none_or(str::is_empty) {
        return Err(validation("Managed login provenance missing."));
    }
    Ok(())
}
impl Store {
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
        tx.execute(
            "UPDATE managed_sessions SET state='quarantined' WHERE id=?1",
            [slot],
        )
        .map_err(storage)?;
        tx.commit().map_err(storage)?;
        secure.write(slot, cache)?;
        if secure.read(slot)? != cache {
            return Err(validation(
                "Secure store readback failed; session quarantined.",
            ));
        }
        cleanup()?;
        c.execute("UPDATE managed_sessions SET state='available',owner=NULL,generation=generation+1 WHERE id=?1 AND owner=?2 AND generation=?3 AND state='quarantined'",params![slot,owner,generation]).map_err(storage)?;
        Ok(())
    }
}
