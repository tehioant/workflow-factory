//! macOS native observations, never caller-supplied approval or ambient Docker context.
//! XNU bsd/sys/sysctl.h: xsw_usage.xsu_encrypted; Docker Desktop uses a local Unix socket.
#[cfg(target_os = "macos")]
pub fn qualify() -> Result<(), String> {
    use std::os::unix::{fs::MetadataExt, net::UnixStream};
    let fail = || {
        "secureStagingUnavailable: macOS requires encrypted swap, FileVault, local Docker Desktop, usable app Keychain, and zero app cores. Guest swap/core checks run again before credential staging. No system settings are changed.".to_string()
    };
    let mut swap: libc::xsw_usage = unsafe { std::mem::zeroed() };
    let mut size = std::mem::size_of_val(&swap);
    if unsafe {
        libc::sysctlbyname(
            c"vm.swapusage".as_ptr(),
            (&mut swap as *mut libc::xsw_usage).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || size != std::mem::size_of_val(&swap)
        || swap.xsu_encrypted == 0
    {
        return Err(fail());
    }
    let socket = UnixStream::connect("/var/run/docker.sock").map_err(|_| fail())?;
    let mut uid = 0;
    let mut gid = 0;
    if unsafe { libc::getpeereid(std::os::fd::AsRawFd::as_raw_fd(&socket), &mut uid, &mut gid) }
        != 0
        || uid != unsafe { libc::geteuid() }
    {
        return Err(fail());
    }
    let home = std::env::var_os("HOME").ok_or_else(fail)?;
    let expected = std::path::PathBuf::from(home).join(".docker/run/docker.sock");
    let actual = std::fs::canonicalize("/var/run/docker.sock").map_err(|_| fail())?;
    let meta = std::fs::metadata(&actual).map_err(|_| fail())?;
    if actual != std::fs::canonicalize(expected).map_err(|_| fail())? || meta.uid() != uid {
        return Err(fail());
    }
    let read = |program: &str, args: &[&str]| -> Result<String, String> {
        let output = std::process::Command::new(program)
            .args(args)
            .env_clear()
            .env("PATH", "/usr/bin:/bin:/usr/local/bin")
            .env("HOME", "/var/empty")
            .env("DOCKER_CONFIG", "/var/empty/workflow-factory-no-config")
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|_| fail())?;
        if !output.status.success() || output.stdout.len() > 16384 {
            return Err(fail());
        }
        String::from_utf8(output.stdout).map_err(|_| fail())
    };
    let vault = read("/usr/bin/fdesetup", &["status"])?;
    let docker = read(
        "docker",
        &[
            "--host",
            "unix:///var/run/docker.sock",
            "info",
            "--format",
            "{{json .}}",
        ],
    )?;
    if !observations(&vault, &docker) {
        return Err(fail());
    }
    let zero = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &zero) } != 0 {
        return Err(fail());
    }
    // Public sentinel only, in our app namespace: verify actual Keychain write/read/delete.
    let account = format!("qualification-{}", std::process::id());
    let entry = keyring::Entry::new("org.workflow-factory.desktop.managed.v1", &account)
        .map_err(|_| fail())?;
    entry
        .set_secret(b"public secure-mode availability probe")
        .map_err(|_| fail())?;
    let readback = entry.get_secret();
    let removed = entry.delete_credential();
    if readback.ok().as_deref() != Some(b"public secure-mode availability probe".as_slice())
        || removed.is_err()
        || !matches!(entry.get_secret(), Err(keyring::Error::NoEntry))
    {
        return Err(fail());
    }
    Ok(())
}
pub fn observations(vault: &str, docker: &str) -> bool {
    vault.trim() == "FileVault is On."
        && serde_json::from_str::<serde_json::Value>(docker)
            .ok()
            .is_some_and(|v| v["OperatingSystem"] == "Docker Desktop" && v["OSType"] == "linux")
}
pub fn confirmation_script() -> &'static str {
    "on run argv\ndisplay dialog (item 1 of argv) with title \"Workflow Factory · trusted private scope\" buttons {\"Cancel\", \"Authorize this operation\"} default button \"Cancel\" cancel button \"Cancel\"\nend run"
}
#[cfg(target_os = "macos")]
pub fn confirm(scope: &str) -> Result<(), workflow_engine::Error> {
    let text = format!("Authorize only this operation on trusted private infrastructure and private trusted inputs, not public/open-source or untrusted workloads. Credentials are accessible to the workload. Trust macOS, Keychain, Docker Desktop and privileged administrators. Do not hibernate or take host/VM memory snapshots while active. No paid fallback or host grant import.\n\n{scope}");
    let result = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", confirmation_script(), "--", &text])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if result.is_ok_and(|s| s.success()) {
        Ok(())
    } else {
        Err(workflow_engine::Error::Validation {
            message: "Native scoped confirmation declined/unavailable; no authority granted."
                .into(),
        })
    }
}
