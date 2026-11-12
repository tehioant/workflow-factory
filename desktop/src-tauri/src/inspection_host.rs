//! Window-owned bounded subscriptions. Only committed journal metadata enters channels.
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Mutex, OnceLock},
};
use workflow_engine::{
    runs::inspection::{artifacts::Artifacts, view::*, Journal},
    Error,
};
struct Subscription {
    window: String,
    workflow: String,
    run: String,
    channel: tauri::ipc::Channel<Notice>,
    flight: bool,
    observed_high: u64,
    cursor: String,
    generation: String,
    floor: u64,
    next: u64,
    artifact_cursor: Option<(String, usize)>,
    text_cursor: Option<(String, String, usize)>,
}
fn subscriptions() -> &'static Mutex<BTreeMap<String, Subscription>> {
    static S: OnceLock<Mutex<BTreeMap<String, Subscription>>> = OnceLock::new();
    S.get_or_init(Mutex::default)
}
fn denied() -> Error {
    Error::Validation {
        message: "Selected inspection unavailable; reload to resynchronize.".into(),
    }
}
fn token() -> String {
    workflow_engine::runs::inspection::opaque_id()
}
fn verify(s: &Subscription, window: &str, workflow: &str, run: &str) -> Result<(), Error> {
    if s.window != window || s.workflow != workflow || s.run != run {
        return Err(denied());
    }
    Ok(())
}
pub fn close(window: &str, workflow: &str, run: &str, id: &str) -> Result<(), Error> {
    let mut all = subscriptions().lock().map_err(|_| denied())?;
    if let Some(s) = all.get(id) {
        verify(s, window, workflow, run)?;
        all.remove(id);
    }
    Ok(())
}
pub fn release(window: &str) {
    if let Ok(mut all) = subscriptions().lock() {
        all.retain(|_, s| s.window != window);
    }
}
pub fn notify(root: &Path, run: &str) {
    let Ok(p) = Journal::page(root, run, None, 0) else {
        return;
    };
    let Ok(mut all) = subscriptions().lock() else {
        return;
    };
    all.retain(|id, s| {
        if s.run != run {
            return true;
        }
        if s.generation == p.generation && s.floor == p.floor && p.high <= s.observed_high {
            return true;
        }
        if s.flight {
            return true;
        }
        s.flight = true;

        s.channel
            .send(Notice {
                run: run.into(),
                subscription: id.clone(),
                generation: p.generation.clone(),
                floor: p.floor.to_string(),
                high: p.high.to_string(),
            })
            .is_ok()
    });
}
pub fn read(
    root: &Path,
    window: &str,
    workflow: &str,
    run: &str,
    id: &str,
    cursor: Option<&str>,
) -> Result<InspectionPage, Error> {
    workflow_engine::runs::inspection::view::scoped(root, workflow, run)?;
    let mut all = subscriptions().lock().map_err(|_| denied())?;
    let s = all.get_mut(id).ok_or_else(denied)?;
    verify(s, window, workflow, run)?;
    let valid = cursor == Some(s.cursor.as_str());
    let p = Journal::page(
        root,
        run,
        valid.then_some(s.generation.as_str()),
        if valid { s.next } else { 0 },
    );
    s.flight = false;

    s.cursor = token();
    match p {
        Ok(p) => {
            s.generation = p.generation.clone();
            s.floor = p.floor;
            s.observed_high = p.high;
            s.next = p.next;
            Ok(InspectionPage {
                run: run.into(),
                subscription: id.into(),
                cursor: s.cursor.clone(),
                generation: p.generation,
                floor: p.floor.to_string(),
                high: p.high.to_string(),
                resync: p.resync || !valid,
                incomplete: p.incomplete,
                available: true,
                more: p.next < p.high,
                entries: p
                    .entries
                    .into_iter()
                    .map(|e| LogEntry {
                        sequence: e.sequence.to_string(),
                        block: e.block,
                        kind: e.kind,
                        text: e.text,
                    })
                    .collect(),
            })
        }
        Err(_) => Ok(InspectionPage {
            run: run.into(),
            subscription: id.into(),
            cursor: s.cursor.clone(),
            generation: String::new(),
            floor: "0".into(),
            high: "0".into(),
            resync: true,
            incomplete: true,
            available: false,
            more: false,
            entries: vec![],
        }),
    }
}
pub fn subscribe(
    root: &Path,
    window: &str,
    workflow: &str,
    run: &str,
    channel: tauri::ipc::Channel<Notice>,
) -> Result<InspectionPage, Error> {
    if window != "main" {
        return Err(denied());
    }
    workflow_engine::runs::inspection::view::scoped(root, workflow, run)?;
    let id = token();
    {
        let mut all = subscriptions().lock().map_err(|_| denied())?;
        if all.len() >= 8 {
            return Err(denied());
        }
        all.insert(
            id.clone(),
            Subscription {
                window: window.into(),
                workflow: workflow.into(),
                run: run.into(),
                channel,
                flight: false,
                observed_high: 0,
                cursor: token(),
                generation: String::new(),
                floor: 0,
                next: 0,
                artifact_cursor: None,
                text_cursor: None,
            },
        );
    }
    // Register first; read holds notification exclusion across the durable snapshot.
    let result = read(root, window, workflow, run, &id, None);
    if result.is_err() {
        let _ = close(window, workflow, run, &id);
    }
    result
}
pub fn artifacts(
    root: &Path,
    window: &str,
    workflow: &str,
    run: &str,
    id: &str,
    cursor: Option<&str>,
) -> Result<ArtifactPage, Error> {
    workflow_engine::runs::inspection::view::scoped(root, workflow, run)?;
    let mut all = subscriptions().lock().map_err(|_| denied())?;
    let s = all.get_mut(id).ok_or_else(denied)?;
    verify(s, window, workflow, run)?;
    let start = match cursor {
        None => 0,
        Some(c) => s
            .artifact_cursor
            .as_ref()
            .filter(|(t, _)| t == c)
            .map(|(_, n)| *n)
            .ok_or_else(denied)?,
    };
    let a = match Artifacts::read(root, run) {
        Ok(a) => a,
        Err(_) => {
            return Ok(ArtifactPage {
                run: run.into(),
                generation: String::new(),
                available: false,
                complete: false,
                cursor: None,
                entries: vec![],
            })
        }
    };
    let end = (start + 20).min(a.files.len());
    if start > end {
        return Err(denied());
    }
    let next = (end < a.files.len()).then(token);
    s.artifact_cursor = next.as_ref().map(|t| (t.clone(), end));
    Ok(ArtifactPage {
        run: run.into(),
        generation: a.generation,
        available: true,
        complete: a.complete,
        cursor: next,
        entries: a.files[start..end]
            .iter()
            .map(|a| ArtifactItem {
                id: a.id.clone(),
                name: a.name.clone(),
            })
            .collect(),
    })
}
#[allow(clippy::too_many_arguments)]
pub fn artifact(
    root: &Path,
    window: &str,
    workflow: &str,
    run: &str,
    sub: &str,
    generation: &str,
    id: &str,
    cursor: Option<&str>,
) -> Result<ArtifactText, Error> {
    workflow_engine::runs::inspection::view::scoped(root, workflow, run)?;
    let mut all = subscriptions().lock().map_err(|_| denied())?;
    let s = all.get_mut(sub).ok_or_else(denied)?;
    verify(s, window, workflow, run)?;
    let start = match cursor {
        None => 0,
        Some(c) => s
            .text_cursor
            .as_ref()
            .filter(|(t, i, _)| t == c && i == id)
            .map(|(_, _, n)| *n)
            .ok_or_else(denied)?,
    };
    let a = Artifacts::read(root, run)?;
    if a.generation != generation {
        return Err(denied());
    }
    let f = a.files.iter().find(|f| f.id == id).ok_or_else(denied)?;
    if start > f.text.len() || !f.text.is_char_boundary(start) {
        return Err(denied());
    }
    let mut end = (start + 32768).min(f.text.len());
    while !f.text.is_char_boundary(end) {
        end -= 1;
    }
    let next = (end < f.text.len()).then(token);
    s.text_cursor = next.as_ref().map(|t| (t.clone(), id.into(), end));
    Ok(ArtifactText {
        run: run.into(),
        generation: generation.into(),
        id: id.into(),
        text: f.text[start..end].into(),
        cursor: next,
    })
}
