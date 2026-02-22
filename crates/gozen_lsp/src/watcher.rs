// File system watcher for incremental project graph and symbol index updates.
//
// Uses the `notify` crate to watch for .gd, .tscn, .tres, and project.godot changes.
// Events are debounced (100ms) and dispatched to update the project graph and symbol index.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::{recommended_watcher, Event, EventKind, RecursiveMode, Watcher};
use tokio::sync::mpsc;

const WATCHER_QUEUE_CAPACITY: usize = 2048;

/// Represents a file change event relevant to the project.
#[derive(Debug)]
pub enum ProjectFileEvent {
    /// A .gd script was created or modified.
    ScriptChanged(PathBuf),
    /// A .gdshader shader was created or modified.
    ShaderChanged(PathBuf),
    /// A .tscn scene was created or modified.
    SceneChanged(PathBuf),
    /// A .tres resource was created or modified.
    ResourceChanged(PathBuf),
    /// project.godot was modified.
    ProjectSettingsChanged,
    /// A file was removed.
    FileRemoved(PathBuf),
}

/// Classify a file system event into a ProjectFileEvent, if relevant.
fn classify_event(event: &Event, project_root: &Path) -> Vec<ProjectFileEvent> {
    let mut out = Vec::new();

    for path in &event.paths {
        // Skip .godot directory
        if path
            .strip_prefix(project_root)
            .ok()
            .and_then(|rel| rel.to_str())
            .map(|s| s.starts_with(".godot") || s.starts_with(".git"))
            .unwrap_or(false)
        {
            continue;
        }

        let is_remove = matches!(event.kind, EventKind::Remove(_));

        if is_remove {
            out.push(ProjectFileEvent::FileRemoved(path.clone()));
            continue;
        }

        // Only handle Create and Modify events
        if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str());
        let file_name = path.file_name().and_then(|n| n.to_str());

        match ext {
            Some("gd") => out.push(ProjectFileEvent::ScriptChanged(path.clone())),
            Some("gdshader") => out.push(ProjectFileEvent::ShaderChanged(path.clone())),
            Some("tscn") => out.push(ProjectFileEvent::SceneChanged(path.clone())),
            Some("tres") => out.push(ProjectFileEvent::ResourceChanged(path.clone())),
            _ => {
                if file_name == Some("project.godot") {
                    out.push(ProjectFileEvent::ProjectSettingsChanged);
                }
            }
        }
    }

    out
}

/// Start watching the project root and return a channel receiver for events.
/// The watcher handle is returned and must be kept alive for the duration of watching.
pub fn start_watching(
    project_root: PathBuf,
) -> anyhow::Result<(impl Watcher, mpsc::Receiver<ProjectFileEvent>)> {
    let (tx, rx) = mpsc::channel(WATCHER_QUEUE_CAPACITY);
    let root = Arc::new(project_root.clone());

    let mut watcher = recommended_watcher(move |res: notify::Result<Event>| {
        if let Ok(event) = res {
            let events = classify_event(&event, &root);
            for evt in events {
                let _ = tx.try_send(evt);
            }
        }
    })?;

    watcher.watch(&project_root, RecursiveMode::Recursive)?;

    Ok((watcher, rx))
}
