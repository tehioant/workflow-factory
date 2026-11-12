//! Private bounded selected diagnostics. No credentials, runtime, recovery or generic path API.
use crate::{output::Publication, storage, validation, Error};
use serde::{Deserialize, Serialize};
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
};
const LIMIT: usize = 1_048_576;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub sequence: u64,
    pub block: String,
    pub epoch: String,
    pub kind: String,
    pub text: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Journal {
    pub version: u32,
    pub run: String,
    pub generation: String,
    pub floor: u64,
    pub high: u64,
    pub incomplete: bool,
    pub entries: Vec<Entry>,
}
#[derive(Debug)]
pub struct Page {
    pub generation: String,
    pub floor: u64,
    pub high: u64,
    pub entries: Vec<Entry>,
    pub next: u64,
    pub resync: bool,
    pub incomplete: bool,
}
fn private(file: &File, directory: bool) -> Result<(), Error> {
    let m = file.metadata().map_err(storage)?;
    if m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || if directory {
            !m.is_dir()
        } else {
            !m.is_file() || m.nlink() != 1
        }
    {
        return Err(validation("Private inspection storage unavailable."));
    }
    Ok(())
}
fn open(parent: &File, name: &str, flags: i32) -> Result<File, Error> {
    let name = CString::new(name).map_err(storage)?;
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o600,
        )
    };
    if fd < 0 {
        return Err(storage(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
fn directory(root: &Path, run: &str, create: bool) -> Result<File, Error> {
    if uuid::Uuid::parse_str(run).is_err() || run.len() != 36 {
        return Err(validation("Invalid inspection run identity."));
    }
    use std::os::unix::fs::OpenOptionsExt;
    let mut parent = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(root)
        .map_err(storage)?;
    private(&parent, true)?;
    for name in ["inspection", run] {
        if create {
            let n = CString::new(name).map_err(storage)?;
            if unsafe { libc::mkdirat(parent.as_raw_fd(), n.as_ptr(), 0o700) } != 0
                && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(storage("Inspection directory unavailable."));
            }
        }
        parent = open(&parent, name, libc::O_RDONLY | libc::O_DIRECTORY)?;
        private(&parent, true)?;
    }
    Ok(parent)
}
fn load(dir: &File, run: &str) -> Result<Journal, Error> {
    let mut file = open(dir, "journal", libc::O_RDONLY)?;
    private(&file, false)?;
    let before = file.metadata().map_err(storage)?;
    if before.len() > LIMIT as u64 {
        return Err(validation("Inspection journal limit exceeded."));
    }
    let mut bytes = Vec::new();
    (&mut file)
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(storage)?;
    let after = file.metadata().map_err(storage)?;
    if bytes.len() > LIMIT
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || before.ctime() != after.ctime()
        || before.ctime_nsec() != after.ctime_nsec()
    {
        return Err(validation("Inspection journal changed."));
    }
    let journal: Journal =
        serde_json::from_slice(&bytes).map_err(|_| validation("Invalid inspection journal."))?;
    if journal.version != 1
        || journal.run != run
        || uuid::Uuid::parse_str(&journal.generation).is_err()
        || journal.floor > journal.high
        || journal.entries.len() > 4096
        || journal
            .entries
            .iter()
            .enumerate()
            .any(|(i, e)| e.sequence != journal.floor + i as u64 + 1 || e.text.len() > 65536)
        || journal.high != journal.floor + journal.entries.len() as u64
    {
        return Err(validation(
            "Inspection journal identity or sequence mismatch.",
        ));
    }
    Ok(journal)
}
impl Journal {
    pub fn commit(root: &Path, run: &str, publication: &Publication) -> Result<(), Error> {
        if publication.identity.run_id != run {
            return Err(validation("Cross-run diagnostic publication refused."));
        }
        let dir = directory(root, run, true)?;
        let exists = unsafe {
            libc::faccessat(
                dir.as_raw_fd(),
                c"journal".as_ptr(),
                libc::F_OK,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        } == 0;
        let mut journal = if exists {
            load(&dir, run)?
        } else {
            Self {
                version: 1,
                run: run.into(),
                generation: uuid::Uuid::new_v4().to_string(),
                floor: 0,
                high: 0,
                incomplete: false,
                entries: vec![],
            }
        };
        journal.incomplete |= publication.incomplete;
        for record in &publication.records {
            let text = match &record.command {
                Some(command) => format!("{command}\n{}", record.text),
                None => record.text.clone(),
            };
            if text.len() > 65536 {
                return Err(validation("Inspection record limit exceeded."));
            }
            journal.high = journal
                .high
                .checked_add(1)
                .ok_or_else(|| validation("Inspection sequence exhausted."))?;
            let entry = Entry {
                sequence: journal.high,
                block: publication.identity.block_id.clone(),
                epoch: publication.identity.epoch.clone(),
                kind: record.kind.clone(),
                text,
            };
            if serde_json::to_vec(&entry).map_err(storage)?.len() > 32768 {
                return Err(validation("Inspection page record limit exceeded."));
            }
            journal.entries.push(entry);
        }
        let bytes = loop {
            let bytes = serde_json::to_vec(&journal).map_err(storage)?;
            if bytes.len() <= LIMIT && journal.entries.len() <= 4096 {
                break bytes;
            }
            if journal.entries.is_empty() {
                return Err(validation("Inspection metadata limit exceeded."));
            }
            journal.entries.remove(0);
            journal.floor += 1;
            journal.incomplete = true;
        };
        let mut file = open(
            &dir,
            "pending",
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
        )?;
        private(&file, false)?;
        file.write_all(&bytes).map_err(storage)?;
        file.sync_all().map_err(storage)?;
        if unsafe {
            libc::renameat(
                dir.as_raw_fd(),
                c"pending".as_ptr(),
                dir.as_raw_fd(),
                c"journal".as_ptr(),
            )
        } != 0
        {
            return Err(storage("Inspection atomic commit failed."));
        }
        dir.sync_all().map_err(storage)?;
        Ok(())
    }
    pub fn page(
        root: &Path,
        run: &str,
        generation: Option<&str>,
        after: u64,
    ) -> Result<Page, Error> {
        let journal = load(&directory(root, run, false)?, run)?;
        let resync = generation != Some(journal.generation.as_str())
            || after < journal.floor
            || after > journal.high;
        let start = if resync { journal.floor } else { after };
        let mut bytes = 0;
        let mut entries = vec![];
        let mut next = start;
        for entry in journal.entries.iter().filter(|e| e.sequence > start) {
            let length = serde_json::to_vec(entry).map_err(storage)?.len();
            if bytes + length > 32768 || entries.len() >= 32 {
                break;
            }
            bytes += length;
            next = entry.sequence;
            entries.push(entry.clone());
        }
        Ok(Page {
            generation: journal.generation,
            floor: journal.floor,
            high: journal.high,
            entries,
            next,
            resync,
            incomplete: journal.incomplete,
        })
    }
}
