//! Descriptor-relative, no-follow reads of one existing Codex file, never a scan.
use crate::auth::{CredentialAvailability as A, CredentialSource, MAX_AUTH_BYTES};
use std::path::PathBuf;
pub struct CodexFile {
    pub home: PathBuf,
}
pub fn safe_metadata(
    uid: u32,
    mode: u32,
    owner: u32,
    private: bool,
    regular: bool,
    links: u64,
    size: u64,
) -> bool {
    if private && uid != owner || !private && uid != 0 && uid != owner || mode & 0o7022 != 0 {
        return false;
    }
    if private && mode & 0o077 != 0 {
        return false;
    }
    if regular {
        mode & 0o111 == 0 && links == 1 && size <= MAX_AUTH_BYTES as u64
    } else {
        mode & 0o100 != 0
    }
}
#[cfg(unix)]
impl CredentialSource for CodexFile {
    fn read(&mut self) -> Result<Vec<u8>, A> {
        use std::{
            ffi::CString,
            fs::File,
            io::Read,
            os::{
                fd::{AsRawFd, FromRawFd},
                unix::{ffi::OsStrExt, fs::MetadataExt},
            },
            path::Component,
        };
        fn error(e: std::io::Error) -> A {
            match e.raw_os_error() {
                Some(libc::ENOENT) => A::Missing,
                Some(libc::ELOOP | libc::ENOTDIR) => A::Unsafe,
                _ => A::Inaccessible,
            }
        }
        fn open(parent: i32, name: &std::ffi::CStr, directory: bool) -> Result<File, A> {
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_CLOEXEC
                | libc::O_NONBLOCK
                | if directory { libc::O_DIRECTORY } else { 0 };
            // All descriptors are owned, anchored to the previously opened directory.
            let fd = unsafe { libc::openat(parent, name.as_ptr(), flags) };
            if fd < 0 {
                Err(error(std::io::Error::last_os_error()))
            } else {
                Ok(unsafe { File::from_raw_fd(fd) })
            }
        }
        if !self.home.is_absolute() {
            return Err(A::Unsafe);
        }
        let owner = unsafe { libc::geteuid() };
        let mut directory = open(libc::AT_FDCWD, c"/", true)?;
        for component in self
            .home
            .components()
            .chain([Component::Normal(std::ffi::OsStr::new(".codex"))])
        {
            let Component::Normal(name) = component else {
                if component == Component::RootDir {
                    continue;
                }
                return Err(A::Unsafe);
            };
            let name = CString::new(name.as_bytes()).map_err(|_| A::Unsafe)?;
            let next = open(directory.as_raw_fd(), &name, true)?;
            let m = next.metadata().map_err(error)?;
            if !m.is_dir()
                || !safe_metadata(
                    m.uid(),
                    m.mode(),
                    owner,
                    name.as_bytes() == b".codex",
                    false,
                    m.nlink(),
                    m.len(),
                )
            {
                return Err(A::Unsafe);
            }
            directory = next;
        }
        let mut file = open(directory.as_raw_fd(), c"auth.json", false)?;
        let before = file.metadata().map_err(error)?;
        if !before.is_file()
            || !safe_metadata(
                before.uid(),
                before.mode(),
                owner,
                true,
                true,
                before.nlink(),
                before.len(),
            )
        {
            return Err(A::Unsafe);
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_AUTH_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        let after = file.metadata().map_err(error)?;
        if bytes.len() > MAX_AUTH_BYTES
            || bytes.len() as u64 != before.len()
            || before.len() != after.len()
            || before.mtime() != after.mtime()
            || before.mtime_nsec() != after.mtime_nsec()
            || before.ctime() != after.ctime()
            || before.ctime_nsec() != after.ctime_nsec()
        {
            bytes.fill(0);
            return Err(A::Unsafe);
        }
        Ok(bytes)
    }
}
#[cfg(not(unix))]
impl CredentialSource for CodexFile {
    fn read(&mut self) -> Result<Vec<u8>, A> {
        Err(A::Unsupported)
    }
}
