use super::*;
use ts_rs::TS;
#[derive(Clone, Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    pub run: String,
    pub subscription: String,
    pub generation: String,
    pub floor: String,
    pub high: String,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct InspectionPage {
    pub run: String,
    pub subscription: String,
    pub cursor: String,
    pub generation: String,
    pub floor: String,
    pub high: String,
    pub resync: bool,
    pub incomplete: bool,
    pub available: bool,
    pub more: bool,
    pub entries: Vec<LogEntry>,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub sequence: String,
    pub block: String,
    pub kind: String,
    pub text: String,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactPage {
    pub run: String,
    pub generation: String,
    pub available: bool,
    pub complete: bool,
    pub cursor: Option<String>,
    pub entries: Vec<ArtifactItem>,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactItem {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactText {
    pub run: String,
    pub generation: String,
    pub id: String,
    pub text: String,
    pub cursor: Option<String>,
}
/// No initialization, migration, permissions changes, execution, or recovery.
pub fn scoped(root: &Path, workflow: &str, run: &str) -> Result<(), Error> {
    let parent = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(storage)?;
    private(&parent, true)?;
    let db = open(
        &parent,
        "definitions.sqlite3",
        libc::O_RDONLY | libc::O_NONBLOCK,
    )?;
    private(&db, false)?;
    let c = rusqlite::Connection::open_with_flags(
        format!("/dev/fd/{}", db.as_raw_fd()),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .map_err(storage)?;
    c.busy_timeout(std::time::Duration::from_millis(250))
        .map_err(storage)?;
    let version: u32 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(storage)?;
    if version != 3 {
        return Err(validation("Captured schema unavailable."));
    }
    crate::runs::history::detail(&c, workflow, run).map(|_| ())
}
use std::os::unix::fs::OpenOptionsExt;
