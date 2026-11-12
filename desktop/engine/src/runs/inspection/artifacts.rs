//! Immutable, bounded sanitized text capture. Public readers accept identities, never paths.
use super::*;
use ts_rs::TS;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifact {
    pub id: String,
    pub name: String,
    pub text: String,
}
#[derive(Debug, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Artifacts {
    pub device: u64,
    pub inode: u64,
    pub run: String,
    pub generation: String,
    pub block: String,
    pub epoch: String,
    pub complete: bool,
    pub files: Vec<Artifact>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GuestFile {
    name: String,
    text: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Guest {
    version: u32,
    identity: crate::output::DiagnosticIdentity,
    complete: bool,
    files: Vec<GuestFile>,
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 2064
        && name.split('/').count() <= 16
        && name.split('/').all(|s| {
            !s.is_empty()
                && s != "."
                && s != ".."
                && s.len() <= 128
                && !s.contains('\\')
                && !s.chars().any(char::is_control)
        })
}
impl Artifacts {
    pub fn commit(
        root: &Path,
        identity: &crate::output::DiagnosticIdentity,
        bytes: &[u8],
        secrets: &[String],
    ) -> Result<(), Error> {
        if bytes.len() > LIMIT
            || identity.block_id.is_empty()
            || identity.block_id.len() > 128
            || !identity
                .block_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || identity.epoch.is_empty()
            || identity.epoch.len() > 128
            || identity.epoch.chars().any(char::is_control)
        {
            return Err(validation("Artifact transport or identity limit exceeded."));
        }
        let guest: Guest = serde_json::from_slice(bytes)
            .map_err(|_| validation("Artifact capture unavailable."))?;
        if guest.version != 1
            || guest.identity != *identity
            || !guest.complete
            || guest.files.len() > 64
        {
            return Err(validation("Artifact capture scope unavailable."));
        }
        let mut total = 0;
        let mut names = std::collections::BTreeSet::new();
        let mut files = vec![];
        for f in guest.files {
            if !valid_name(&f.name)
                || !names.insert(f.name.clone())
                || f.text.len() > 65536
                || f.text
                    .chars()
                    .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                || secrets
                    .iter()
                    .any(|s| s.is_empty() || f.name.contains(s) || f.text.contains(s))
            {
                return Err(validation("Artifact text or name unavailable."));
            }
            total += f.text.len();
            if total > 524288 {
                return Err(validation("Artifact aggregate limit exceeded."));
            }
            files.push(Artifact {
                id: uuid::Uuid::new_v4().to_string(),
                name: f.name,
                text: f.text,
            });
        }
        let mut capture = Self {
            device: 0,
            inode: 0,
            run: identity.run_id.clone(),
            generation: uuid::Uuid::new_v4().to_string(),
            block: identity.block_id.clone(),
            epoch: identity.epoch.clone(),
            complete: true,
            files,
        };

        let dir = directory(root, &identity.run_id, true)?;
        let pending = format!("artifacts-{}", uuid::Uuid::new_v4());
        let mut file = open(
            &dir,
            &pending,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        private(&file, false)?;
        let metadata = file.metadata().map_err(storage)?;
        capture.device = metadata.dev();
        capture.inode = metadata.ino();
        let bytes = serde_json::to_vec(&capture).map_err(storage)?;
        if bytes.len() > LIMIT {
            return Err(validation("Artifact retention limit exceeded."));
        }
        file.write_all(&bytes).map_err(storage)?;
        file.sync_all().map_err(storage)?;
        // linkat is atomic no-replace: repeated captures cannot replace committed content.
        let name = CString::new(pending).map_err(storage)?;
        let result = unsafe {
            libc::linkat(
                dir.as_raw_fd(),
                name.as_ptr(),
                dir.as_raw_fd(),
                c"artifacts".as_ptr(),
                0,
            )
        };
        unsafe {
            libc::unlinkat(dir.as_raw_fd(), name.as_ptr(), 0);
        }
        if result != 0 {
            return Err(validation(
                "Artifact capture already committed or unavailable.",
            ));
        }
        dir.sync_all().map_err(storage)?;
        Ok(())
    }
    pub fn read(root: &Path, run: &str) -> Result<Self, Error> {
        let dir = directory(root, run, false)?;
        let mut file = open(&dir, "artifacts", libc::O_RDONLY | libc::O_NONBLOCK)?;
        private(&file, false)?;
        let before = file.metadata().map_err(storage)?;
        if before.len() > LIMIT as u64 {
            return Err(validation("Artifact manifest oversized."));
        }
        let mut bytes = vec![];
        (&mut file)
            .take(LIMIT as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(storage)?;
        private(&file, false)?;
        let after = file.metadata().map_err(storage)?;
        if bytes.len() > LIMIT
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            return Err(validation("Artifact capture changed."));
        }
        let result: Self = serde_json::from_slice(&bytes)
            .map_err(|_| validation("Artifact manifest unavailable."))?;
        let mut ids = std::collections::BTreeSet::new();
        let mut names = std::collections::BTreeSet::new();
        if result.device != before.dev()
            || result.inode != before.ino()
            || result.run != run
            || !result.complete
            || uuid::Uuid::parse_str(&result.generation).is_err()
            || result.files.len() > 64
            || result.files.iter().any(|f| {
                uuid::Uuid::parse_str(&f.id).is_err()
                    || !ids.insert(&f.id)
                    || !names.insert(&f.name)
                    || !valid_name(&f.name)
                    || f.text.len() > 65536
            })
            || result.files.iter().map(|f| f.text.len()).sum::<usize>() > 524288
        {
            return Err(validation("Artifact manifest scope unavailable."));
        }
        Ok(result)
    }
}
