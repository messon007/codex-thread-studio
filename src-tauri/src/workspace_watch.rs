//! Non-recursive watches for the directories currently visible in the Files rail.
use axum::extract::ws::{Message, WebSocket};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Deserialize)]
struct Request {
    root: PathBuf,
    paths: Vec<String>,
}

fn directories(request: Request) -> Result<Vec<(String, PathBuf)>, String> {
    if request.paths.len() > 128 {
        return Err("Too many expanded directories (maximum 128)".into());
    }
    let root = request.root.canonicalize().map_err(|e| e.to_string())?;
    if !root.is_dir() {
        return Err("Project root is not a directory".into());
    }
    let mut output = Vec::new();
    let mut seen = BTreeSet::new();
    for relative in request.paths {
        if Path::new(&relative).is_absolute() {
            return Err("Watch paths must be relative".into());
        }
        let path = root
            .join(&relative)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !path.starts_with(&root) {
            return Err("Watch directory is outside project root".into());
        }
        if !path.is_dir() {
            return Err("Watch path is not a directory".into());
        }
        if seen.insert(path.clone()) {
            output.push((relative, path));
        }
    }
    Ok(output)
}

fn changed_directories(watched: &[(String, PathBuf)], event: &notify::Event) -> Vec<String> {
    if event.kind.is_access() {
        return Vec::new();
    }
    watched
        .iter()
        .filter(|(_, directory)| {
            event.paths.is_empty()
                || event
                    .paths
                    .iter()
                    .any(|path| path == directory || path.parent() == Some(directory.as_path()))
        })
        .map(|(relative, _)| relative.clone())
        .collect()
}

pub async fn serve(mut socket: WebSocket) {
    let (signal, mut changes) = tokio::sync::mpsc::channel::<()>(1);
    let dirty = Arc::new(Mutex::new(BTreeSet::<String>::new()));
    let mut watcher: Option<RecommendedWatcher> = None;
    loop {
        tokio::select! {
            message = socket.recv() => {
                let text = match message {
                    Some(Ok(Message::Text(text))) => text,
                    Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                    _ => break,
                };
                if text.len() > 64 * 1024 { break; }
                let result = serde_json::from_str::<Request>(&text).map_err(|e| e.to_string()).and_then(directories);
                let watched = match result {
                    Ok(value) => value,
                    Err(message) => {
                        let _ = socket.send(Message::Text(serde_json::json!({"type":"error", "message":message}).to_string().into())).await;
                        break;
                    }
                };
                // Drop old OS handles before installing the replacement subscription.
                drop(watcher.take());
                if let Ok(mut dirty) = dirty.lock() { dirty.clear(); }
                let callback_paths = watched.clone();
                let callback_dirty = dirty.clone();
                let callback_signal = signal.clone();
                let created = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                    let paths = match event {
                        Ok(event) => changed_directories(&callback_paths, &event),
                        Err(_) => callback_paths.iter().map(|(relative, _)| relative.clone()).collect(),
                    };
                    if paths.is_empty() { return; }
                    if let Ok(mut dirty) = callback_dirty.lock() { dirty.extend(paths); }
                    let _ = callback_signal.try_send(());
                });
                let installed = created.and_then(|mut active| {
                    for (_, path) in &watched { active.watch(path, RecursiveMode::NonRecursive)?; }
                    Ok(active)
                });
                match installed {
                    Ok(active) => watcher = Some(active),
                    Err(error) => {
                        let _ = socket.send(Message::Text(serde_json::json!({"type":"error", "message":error.to_string()}).to_string().into())).await;
                        break;
                    }
                }
                let paths: Vec<_> = watched.iter().map(|(relative, _)| relative).collect();
                if socket.send(Message::Text(serde_json::json!({"type":"ready", "paths":paths}).to_string().into())).await.is_err() { break; }
            }
            changed = changes.recv() => {
                if changed.is_none() { break; }
                tokio::time::sleep(Duration::from_millis(150)).await;
                while changes.try_recv().is_ok() {}
                let paths = dirty.lock().map(|mut dirty| std::mem::take(&mut *dirty)).unwrap_or_default();
                if !paths.is_empty() && socket.send(Message::Text(serde_json::json!({"type":"changed", "paths":paths}).to_string().into())).await.is_err() { break; }
            }
        }
    }
    drop(watcher);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ignores_reads_and_only_invalidates_affected_directories() {
        let watched = vec![
            ("".into(), PathBuf::from("/root")),
            ("sub".into(), PathBuf::from("/root/sub")),
        ];
        let event = notify::Event::new(notify::EventKind::Create(notify::event::CreateKind::File))
            .add_path(PathBuf::from("/root/sub/file"));
        assert_eq!(changed_directories(&watched, &event), vec!["sub"]);
        let read = notify::Event::new(notify::EventKind::Access(notify::event::AccessKind::Read))
            .add_path(PathBuf::from("/root/file"));
        assert!(changed_directories(&watched, &read).is_empty());
    }
    #[test]
    fn bounds_watch_requests_and_rejects_escape() {
        assert!(directories(Request {
            root: std::env::temp_dir(),
            paths: vec!["".into(); 129]
        })
        .is_err());
        assert!(directories(Request {
            root: std::env::temp_dir(),
            paths: vec!["..".into()]
        })
        .is_err());
    }
}
