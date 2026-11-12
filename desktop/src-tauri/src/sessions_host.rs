//! OS-native secret store, app namespace only. No file or host-Codex fallback.
use workflow_engine::{
    sessions::{SecureStore, CACHE_LIMIT},
    Error,
};
pub struct NativeSecureStore;
fn unavailable() -> Error {
    Error::Storage{message:"Native secure storage unavailable, locked, denied or too small. Session remains withheld; no plaintext fallback.".into()}
}
fn entry(slot: &str) -> Result<keyring::Entry, Error> {
    if slot.len() != 36 || workflow_engine::sessions::valid_slot(slot).is_err() {
        return Err(unavailable());
    }
    keyring::Entry::new("org.workflow-factory.desktop.managed.v1", slot).map_err(|_| unavailable())
}
impl SecureStore for NativeSecureStore {
    fn retire(&mut self, slot: &str) -> Result<(), Error> {
        let item = entry(slot)?;
        match item.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err(unavailable()),
        }
        match item.get_secret() {
            Err(keyring::Error::NoEntry) => Ok(()),
            _ => Err(unavailable()),
        }
    }
    fn read(&mut self, slot: &str) -> Result<Vec<u8>, Error> {
        let value = entry(slot)?.get_secret().map_err(|_| unavailable())?;
        if value.len() > CACHE_LIMIT {
            return Err(unavailable());
        }
        Ok(value)
    }
    fn write(&mut self, slot: &str, cache: &[u8]) -> Result<(), Error> {
        if cache.len() > CACHE_LIMIT {
            return Err(unavailable());
        }
        entry(slot)?.set_secret(cache).map_err(|_| unavailable())
    }
}
