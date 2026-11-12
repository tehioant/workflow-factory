//! Selected volatile continuation; no discovery, execution, staging or admission.
use super::*;
use crate::sessions::{CompletionReceipt, Lease, SecureStore};
pub struct CleanupContext {
    resources: Resources,
    lease: Lease,
    checkpoint: Option<Vec<u8>>,
    removing: bool,
}
impl CleanupContext {
    pub fn new(resources: Resources, lease: Lease) -> Self {
        Self {
            resources,
            lease,
            checkpoint: None,
            removing: false,
        }
    }
}
impl<P: Pipe> Sandbox<P> {
    /// Real producer only: current export after positive writer stop and exact live epoch.
    pub fn checkpoint_cleanup(&mut self, context: &mut CleanupContext) -> Result<Vec<u8>, Error> {
        let r = &context.resources;
        if context.removing
            || r.peer.is_some()
            || self.live_epoch(r)? != r.started_at.as_deref().unwrap_or("")
        {
            return Err(validation("Cleanup checkpoint epoch uncertain."));
        }
        self.stop_writers(r)?;
        let cache = self.export(r)?;
        context.checkpoint = Some(cache.clone());
        Ok(cache)
    }
    pub fn finish_cleanup(
        &mut self,
        store: &crate::Store,
        lease: &Lease,
        context: &mut CleanupContext,
        secure: &mut impl SecureStore,
        reusable: bool,
    ) -> Result<CompletionReceipt, Error> {
        let r = context.resources.clone();
        if r.owner != lease.owner
            || context.lease.slot != lease.slot
            || context.lease.owner != lease.owner
            || context.lease.generation != lease.generation
            || r.peer.is_some()
            || r.started_at.is_none()
        {
            return Err(validation("Cleanup context uncertain; ownership held."));
        }
        let records = store.cleanup_records(lease)?;
        self.validate_cleanup_journal(&r, &records)?;
        if self.listed("container", &r.container)? {
            if self.live_epoch(&r)? != r.started_at.as_deref().unwrap_or("") {
                return Err(validation("Keeper epoch changed; cleanup held."));
            }
            context.removing = false;
            self.checkpoint_cleanup(context)?;
        } else if !context.removing {
            return Err(validation(
                "Keeper absent without current-cache checkpoint proof.",
            ));
        }
        let cache = context
            .checkpoint
            .as_ref()
            .ok_or_else(|| validation("Current-cache proof unavailable; ownership held."))?;
        store.finish_owned_session_receipt(lease, cache, secure, reusable, || {
            context.removing = true; // only after exact secure write/readback, before first removal
            self.continue_cleanup(&r, |kind, id| {
                store.cleanup_resource_receipt(lease, kind, id)
            })
        })
    }
    fn validate_cleanup_journal(
        &mut self,
        r: &Resources,
        records: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        let mut required = std::collections::BTreeSet::from([
            r.container.as_str(),
            r.helper.as_str(),
            r.network.as_str(),
            r.outer.as_str(),
        ]);
        if let Some(name) = &r.workspace {
            required.insert(name);
        }
        for (kind, id) in records {
            let removal = matches!(
                kind.as_str(),
                "final-agent-remove"
                    | "helper-remove"
                    | "workspace-remove"
                    | "private-network-remove"
                    | "egress-network-remove"
            );
            let numbered = [
                "work-container-",
                "agent-start-",
                "agent-overlap-",
                "agent-remove-",
            ]
            .iter()
            .any(|p| {
                kind.strip_prefix(p)
                    .is_some_and(|n| n.parse::<usize>().is_ok_and(|n| n < 100))
            });
            if !removal
                && !numbered
                && !matches!(
                    kind.as_str(),
                    "work-container"
                        | "egress-helper"
                        | "private-network"
                        | "egress-network"
                        | "workspace-volume"
                )
            {
                return Err(validation("Unknown cleanup journal; ownership held."));
            }
            let expected = match kind.as_str() {
                "egress-helper" | "helper-remove" => Some(r.helper.as_str()),
                "private-network" | "private-network-remove" => Some(r.network.as_str()),
                "egress-network" | "egress-network-remove" => Some(r.outer.as_str()),
                "workspace-volume" | "workspace-remove" => r.workspace.as_deref(),
                "final-agent-remove" => Some(r.container.as_str()),
                _ => None,
            };
            if matches!(kind.as_str(), "workspace-volume" | "workspace-remove")
                && r.workspace.is_none()
            {
                return Err(validation("Unexpected workspace journal."));
            }
            match id.as_deref() {
                None if removal => {}
                None => return Err(validation("Unacknowledged resource intent; cleanup held.")),
                Some(id) if expected.is_some() && expected != Some(id) => {
                    return Err(validation("Foreign cleanup journal identity."))
                }
                Some(id) => {
                    let creation = matches!(
                        kind.as_str(),
                        "work-container"
                            | "egress-helper"
                            | "private-network"
                            | "egress-network"
                            | "workspace-volume"
                    ) || kind.starts_with("work-container-");
                    if creation {
                        required.remove(id);
                    }
                    if expected.is_none() && !opaque(id) {
                        return Err(validation("Invalid recorded agent identity."));
                    }
                    if expected.is_none() && id != r.container {
                        // Historical agents may only be acknowledged absent, never adopted.
                        self.absent_container(id, &Cancel::default())?;
                    }
                }
            }
        }
        if !required.is_empty() {
            return Err(validation("Incomplete owned cleanup journal."));
        }
        Ok(())
    }
    fn listed(&mut self, kind: &str, id: &str) -> Result<bool, Error> {
        let args = match kind {
            "container" => strings(&[
                "container",
                "ls",
                "-a",
                "--no-trunc",
                "--filter",
                &format!("id={id}"),
                "--format",
                "{{.ID}}",
            ]),
            "network" => strings(&[
                "network",
                "ls",
                "--no-trunc",
                "--filter",
                &format!("id={id}"),
                "--format",
                "{{.ID}}",
            ]),
            "volume" => strings(&[
                "volume",
                "ls",
                "--filter",
                &format!("name=^{id}$"),
                "--format",
                "{{.Name}}",
            ]),
            _ => return Err(validation("Unknown cleanup kind.")),
        };
        let out = self.success(&args, &[], 131072)?;
        let text = std::str::from_utf8(&out).map_err(|_| validation("Cleanup listing invalid."))?;
        let ids: Vec<_> = text.split_whitespace().collect();
        if ids.is_empty() {
            return Ok(false);
        }
        if ids != [id] {
            return Err(validation("Cleanup listing identity uncertain."));
        }
        Ok(true)
    }
    fn continue_cleanup(
        &mut self,
        r: &Resources,
        mut receipt: impl FnMut(&str, Option<&str>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        for (kind, tag, id) in [
            (
                "container",
                "final-agent-remove",
                Some(r.container.as_str()),
            ),
            ("container", "helper-remove", Some(r.helper.as_str())),
            ("volume", "workspace-remove", r.workspace.as_deref()),
            (
                "network",
                "private-network-remove",
                Some(r.network.as_str()),
            ),
            ("network", "egress-network-remove", Some(r.outer.as_str())),
        ] {
            let Some(id) = id else { continue };
            receipt(tag, None)?;
            if self.listed(kind, id)? {
                if kind == "volume" {
                    self.inspect_workspace(id, &r.owner)?;
                    let used = self.success(
                        &strings(&[
                            "container",
                            "ls",
                            "-a",
                            "--no-trunc",
                            "--filter",
                            &format!("volume={id}"),
                            "--format",
                            "{{.ID}}",
                        ]),
                        &[],
                        131072,
                    )?;
                    if used.iter().any(|b| !b.is_ascii_whitespace()) {
                        return Err(validation("Workspace still referenced."));
                    }
                } else {
                    let row = self.owned(kind, id, &r.owner)?;
                    if kind == "network"
                        && row["Containers"].as_object().is_none_or(|a| !a.is_empty())
                    {
                        return Err(validation("Network remains in use."));
                    }
                    if id == r.helper {
                        self.helper_mounts(&row)?;
                    }
                    if id == r.container {
                        if row["Image"] != r.image
                            || row["State"]["StartedAt"] != r.started_at.as_deref().unwrap_or("")
                            || row["RestartCount"] != 0
                        {
                            return Err(validation("Keeper epoch changed; cleanup held."));
                        }
                        self.inspect_workspace_mount(r, &row, true)?;
                    }
                }
                let args = if kind == "container" {
                    strings(&["rm", "-f", id])
                } else {
                    strings(&[kind, "rm", id])
                };
                self.success(&args, &[], 131072)?;
            }
            if self.listed(kind, id)? {
                return Err(validation("Owned removal absence unconfirmed."));
            }
            receipt(tag, Some(id))?;
        }
        Ok(())
    }
}
