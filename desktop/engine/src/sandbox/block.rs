use super::*;
const OPTIONS: &str = "rw,exec,nosuid,nodev,size=256m,uid=1000,gid=1000,mode=700";
fn workspace_name(owner: &str) -> String {
    format!("wf-workspace-{owner}")
}
impl<P: Pipe> Sandbox<P> {
    pub(super) fn create_workspace(
        &mut self,
        owner: &str,
        cancel: &Cancel,
        receipt: &mut impl FnMut(&str, Option<&str>) -> Result<(), Error>,
    ) -> Result<String, Error> {
        let name = workspace_name(owner);
        self.volume_absent(&name, owner, cancel)?;
        cancel.check()?;
        receipt("workspace-volume", None)?;
        let out = self.work_success(
            &strings(&[
                "volume",
                "create",
                "--driver",
                "local",
                "--label",
                &format!("org.workflow-factory.owner={owner}"),
                "--opt",
                "type=tmpfs",
                "--opt",
                "device=tmpfs",
                "--opt",
                &format!("o={OPTIONS}"),
                &name,
            ]),
            &[],
            131072,
            cancel,
        )?;
        if std::str::from_utf8(&out).ok().map(str::trim) != Some(name.as_str()) {
            return Err(validation(
                "Workspace create identity unknown; ownership held.",
            ));
        }
        self.inspect_workspace(&name, owner)?;
        cancel.check()?;
        receipt("workspace-volume", Some(&name))?;
        Ok(name)
    }
    pub(super) fn inspect_workspace(&mut self, name: &str, owner: &str) -> Result<(), Error> {
        crate::sessions::valid_slot(owner)?;
        if name != workspace_name(owner) {
            return Err(validation("Workspace name does not match run."));
        }
        let v = self.json(&["volume", "inspect", name])?;
        let a = v
            .as_array()
            .filter(|a| a.len() == 1)
            .ok_or_else(|| validation("Workspace inspection missing."))?;
        let v = &a[0];
        if v["Name"] != name
            || v["Driver"] != "local"
            || v["Scope"] != "local"
            || v["Labels"]["org.workflow-factory.owner"] != owner
            || v["Options"] != serde_json::json!({"type":"tmpfs","device":"tmpfs","o":OPTIONS})
        {
            return Err(validation(
                "Owned local tmpfs workspace unconfirmed; no adoption or fallback.",
            ));
        }
        Ok(())
    }
    fn volume_absent(&mut self, name: &str, owner: &str, cancel: &Cancel) -> Result<(), Error> {
        for filter in [
            format!("name=^{name}$"),
            format!("label=org.workflow-factory.owner={owner}"),
        ] {
            let out = self.work_success(
                &strings(&["volume", "ls", "--filter", &filter, "--format", "{{.Name}}"]),
                &[],
                131072,
                cancel,
            )?;
            if out.iter().any(|c| !c.is_ascii_whitespace()) {
                return Err(validation("Workspace absence unconfirmed; ownership held."));
            }
        }
        Ok(())
    }
    pub(super) fn inspect_workspace_mount(
        &mut self,
        r: &Resources,
        c: &Value,
        started: bool,
    ) -> Result<(), Error> {
        let Some(name) = &r.workspace else {
            return Ok(());
        };
        self.inspect_workspace(name, &r.owner)?;
        let mounts = c["Mounts"]
            .as_array()
            .ok_or_else(|| validation("Workspace mount inspection missing."))?;
        let volumes = mounts
            .iter()
            .filter(|m| m["Type"] != "tmpfs")
            .collect::<Vec<_>>();
        if volumes.len() != 1
            || volumes[0]["Type"] != "volume"
            || volumes[0]["Name"] != *name
            || volumes[0]["Driver"] != "local"
            || volumes[0]["Destination"] != "/workspace"
            || volumes[0]["RW"] != true
            || (started && c["State"]["Running"] != true)
        {
            return Err(validation(
                "Live shared tmpfs workspace continuity unconfirmed.",
            ));
        }
        if c["Config"]["Entrypoint"] != serde_json::json!(["node"])
            || c["Config"]["Cmd"] != serde_json::json!(["/opt/workflow-factory/keeper.mjs"])
        {
            return Err(validation("Inert keeper command unconfirmed."));
        }
        let h = &c["HostConfig"];
        let declared = h["Mounts"]
            .as_array()
            .ok_or_else(|| validation("Explicit workspace mount missing."))?;
        if declared.len() != 1
            || declared[0]["Type"] != "volume"
            || declared[0]["Source"] != *name
            || declared[0]["Target"] != "/workspace"
            || declared[0]["ReadOnly"] != false
            || declared[0]["VolumeOptions"]["NoCopy"] != true
            || !h["Tmpfs"]["/workspace"].is_null()
        {
            return Err(validation("Unexpected declared workspace mount."));
        }
        Ok(())
    }
    fn absent_container(&mut self, id: &str, cancel: &Cancel) -> Result<(), Error> {
        if !opaque(id) {
            return Err(validation("Invalid recorded container."));
        }
        let out = self.work_success(
            &strings(&[
                "container",
                "ls",
                "-a",
                "--no-trunc",
                "--filter",
                &format!("id={id}"),
                "--format",
                "{{.ID}}",
            ]),
            &[],
            131072,
            cancel,
        )?;
        if out.iter().any(|c| !c.is_ascii_whitespace()) {
            return Err(validation("Old agent absence unconfirmed."));
        }
        Ok(())
    }
    /// Prepared boundary only: caller has durably stored successful output and
    /// positively stopped writers/exported CURRENT cache. No executor lives here.
    pub fn switch_block_environment(
        &mut self,
        old: &Resources,
        ordinal: usize,
        cancel: &Cancel,
        checkpoint: impl FnOnce() -> Result<Vec<u8>, Error>,
        mut receipt: impl FnMut(&str, Option<&str>) -> Result<(), Error>,
        qualify: impl FnOnce() -> Result<(), Error>,
    ) -> Result<Resources, Error> {
        if old.workspace.is_none() {
            return Ok(old.clone());
        }
        if ordinal == 0 || ordinal >= 100 || old.peer.is_some() {
            return Err(validation("Invalid prepared block boundary."));
        }
        cancel.check()?;
        self.inspect_work(old)?;
        let cache = checkpoint()?;
        crate::sessions::validate_cache(&cache)?;
        cancel.check()?;
        let kind = format!("work-container-{ordinal}");
        receipt(&kind, None)?;
        self.inspect_workspace(old.workspace.as_deref().unwrap(), &old.owner)?;
        let out = self.work_success(&agent_args(old), &[], 131072, cancel)?;
        let id = std::str::from_utf8(&out)
            .map_err(crate::storage)?
            .trim()
            .to_string();
        if !opaque(&id) || [&old.container, &old.helper].contains(&&id) {
            return Err(validation("Fresh agent identity unconfirmed."));
        }
        cancel.check()?;
        receipt(&kind, Some(&id))?;
        let mut next = Resources {
            container: id.clone(),
            peer: Some(old.container.clone()),
            ..old.clone()
        };
        self.inspect_phase(&next, false)?;
        let start = format!("agent-start-{ordinal}");
        cancel.check()?;
        receipt(&start, None)?;
        self.work_success(&strings(&["start", &id]), &[], 131072, cancel)?;
        self.inspect_work(&next)?;
        let previous = Resources {
            peer: Some(id.clone()),
            ..old.clone()
        };
        self.inspect_work(&previous)?;
        // Fresh credentials/tmp are not copied, nor may image startup execute workspace code.
        self.work_success(&strings(&["exec",&id,"node","-e","const fs=require('node:fs');if(fs.readdirSync('/run/wf-codex').length||fs.readdirSync('/tmp').length)process.exit(1)"]),&[],1024,cancel)?;
        cancel.check()?;
        receipt(&start, Some(&id))?;
        let overlap = format!("agent-overlap-{ordinal}");
        receipt(&overlap, None)?;
        cancel.check()?;
        receipt(&overlap, Some(&id))?;
        let remove = format!("agent-remove-{}", ordinal - 1);
        cancel.check()?;
        receipt(&remove, None)?;
        self.owned("container", &old.container, &old.owner)?;
        self.work_success(&strings(&["rm", "-f", &old.container]), &[], 131072, cancel)?;
        self.absent_container(&old.container, cancel)?;
        cancel.check()?;
        receipt(&remove, Some(&old.container))?;
        next.peer = None;
        self.inspect_work(&next)?;
        cancel.check()?;
        qualify()?;
        cancel.check()?;
        self.absent_container(&old.container, cancel)?;
        self.stage(&next, &cache, cancel)?;
        Ok(next)
    }
    pub fn cleanup_block_resources(
        &mut self,
        r: &Resources,
        cancel: &Cancel,
        mut receipt: impl FnMut(&str, Option<&str>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for (kind, id) in [
            ("final-agent-remove", &r.container),
            ("helper-remove", &r.helper),
        ] {
            cancel.check()?;
            receipt(kind, None)?;
            let row = self.owned("container", id, &r.owner)?;
            if id == &r.helper {
                self.helper_mounts(&row)?;
            }
            self.work_success(&strings(&["rm", "-f", id]), &[], 131072, cancel)?;
            self.absent_container(id, cancel)?;
            cancel.check()?;
            receipt(kind, Some(id))?;
        }
        if let Some(name) = &r.workspace {
            cancel.check()?;
            receipt("workspace-remove", None)?;
            self.inspect_workspace(name, &r.owner)?;
            let used = self.work_success(
                &strings(&[
                    "container",
                    "ls",
                    "-a",
                    "--no-trunc",
                    "--filter",
                    &format!("volume={name}"),
                    "--format",
                    "{{.ID}}",
                ]),
                &[],
                131072,
                cancel,
            )?;
            if used.iter().any(|c| !c.is_ascii_whitespace()) {
                return Err(validation("Workspace still referenced; removal held."));
            }
            self.work_success(&strings(&["volume", "rm", name]), &[], 131072, cancel)?;
            self.volume_absent(name, &r.owner, cancel)?;
            cancel.check()?;
            receipt("workspace-remove", Some(name))?;
        }
        for (kind, id) in [
            ("private-network-remove", &r.network),
            ("egress-network-remove", &r.outer),
        ] {
            cancel.check()?;
            receipt(kind, None)?;
            let n = self.owned("network", id, &r.owner)?;
            if n["Containers"].as_object().is_none_or(|a| !a.is_empty()) {
                return Err(validation("Network remains in use."));
            }
            self.work_success(&strings(&["network", "rm", id]), &[], 131072, cancel)?;
            let out = self.work_success(
                &strings(&[
                    "network",
                    "ls",
                    "--no-trunc",
                    "--filter",
                    &format!("id={id}"),
                    "--format",
                    "{{.ID}}",
                ]),
                &[],
                131072,
                cancel,
            )?;
            if out.iter().any(|c| !c.is_ascii_whitespace()) {
                return Err(validation("Network absence unconfirmed."));
            }
            cancel.check()?;
            receipt(kind, Some(id))?;
        }
        Ok(())
    }
    pub(super) fn helper_mounts(&self, helper: &Value) -> Result<(), Error> {
        if helper["Mounts"].as_array().is_none_or(|a| {
            a.iter()
                .any(|m| m["Type"] != "tmpfs" || m["Destination"] != "/tmp")
        }) || helper["HostConfig"]["Mounts"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
            || helper["HostConfig"]["Tmpfs"]
                .as_object()
                .is_none_or(|a| a.len() != 1 || !a.contains_key("/tmp"))
        {
            return Err(validation(
                "Helper must remain separate from workspace and credentials.",
            ));
        }
        Ok(())
    }
}
fn agent_args(r: &Resources) -> Vec<String> {
    strings(&[
        "create",
        "--label",
        &format!("org.workflow-factory.owner={}", r.owner),
        "--network",
        &r.network,
        "--read-only",
        "--init",
        "--user",
        "1000:1000",
        "--cap-drop",
        "ALL",
        "--security-opt",
        "no-new-privileges",
        "--memory",
        "512m",
        "--memory-swap",
        "512m",
        "--cpus",
        "1",
        "--pids-limit",
        "128",
        "--log-driver",
        "none",
        "--ulimit",
        "core=0:0",
        "--tmpfs",
        "/tmp:rw,noexec,nosuid,nodev,size=64m,uid=1000,gid=1000,mode=700",
        "--dns",
        "127.0.0.1",
        "--tmpfs",
        "/run/wf-codex:rw,noexec,nosuid,nodev,size=16m,uid=1000,gid=1000,mode=700",
        "--mount",
        &format!(
            "type=volume,src={},dst=/workspace,volume-nocopy",
            r.workspace.as_deref().unwrap()
        ),
        "--env",
        "HOME=/run/wf-codex",
        "--env",
        "CODEX_HOME=/run/wf-codex",
        "--env",
        "HTTPS_PROXY=http://wf-egress:3128",
        "--env",
        "HTTP_PROXY=http://wf-egress:3128",
        "--env",
        "NO_PROXY=",
        "--entrypoint",
        "node",
        &r.image,
        "/opt/workflow-factory/keeper.mjs",
    ])
}
