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

#[derive(Clone, Deserialize)]
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

fn shared_directories(
    request: Request,
    shared: &[PathBuf],
) -> Result<Vec<(String, PathBuf)>, String> {
    if request.paths.len() > 128 {
        return Err("Too many expanded directories (maximum 128)".into());
    }
    let root = request.root.to_str().ok_or("Invalid project path")?;
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for path in request.paths {
        let (_, canonical) =
            crate::native_files::directory(root, &path, shared).map_err(|(_, message)| message)?;
        if seen.insert(canonical.clone()) {
            output.push((path, canonical));
        }
    }
    Ok(output)
}
async fn resolve(
    request: Request,
    access: Option<crate::GatewayState>,
) -> Result<Vec<(String, PathBuf)>, String> {
    tokio::task::spawn_blocking(move || match access {
        Some(state) => {
            let shared =
                crate::shared_document_directories(&state).map_err(|(_, message)| message)?;
            shared_directories(request, &shared)
        }
        None => directories(request),
    })
    .await
    .map_err(|_| "Directory watch worker stopped")?
}
fn install(
    old: Option<RecommendedWatcher>,
    watched: Vec<(String, PathBuf)>,
    dirty: Arc<Mutex<BTreeSet<String>>>,
    signal: tokio::sync::mpsc::Sender<()>,
) -> notify::Result<RecommendedWatcher> {
    drop(old);
    if let Ok(mut dirty) = dirty.lock() {
        dirty.clear();
    }
    let callback_paths = watched.clone();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let paths = match event {
            Ok(event) => changed_directories(&callback_paths, &event),
            Err(_) => callback_paths
                .iter()
                .map(|(relative, _)| relative.clone())
                .collect(),
        };
        if paths.is_empty() {
            return;
        }
        if let Ok(mut dirty) = dirty.lock() {
            dirty.extend(paths);
        }
        let _ = signal.try_send(());
    })?;
    for (_, path) in &watched {
        watcher.watch(path, RecursiveMode::NonRecursive)?;
    }
    Ok(watcher)
}
pub async fn serve(socket: WebSocket) {
    serve_with(socket, None).await;
}
pub async fn serve_native(socket: WebSocket, state: crate::GatewayState) {
    serve_with(socket, Some(state)).await;
}
async fn serve_with(mut socket: WebSocket, access: Option<crate::GatewayState>) {
    let (signal, mut changes) = tokio::sync::mpsc::channel::<()>(1);
    let dirty = Arc::new(Mutex::new(BTreeSet::<String>::new()));
    let mut watcher: Option<RecommendedWatcher> = None;
    let mut subscription: Option<(Request, Vec<(String, PathBuf)>)> = None;
    loop {
        tokio::select! {
            message=socket.recv()=>{
                let text=match message {Some(Ok(Message::Text(text)))=>text,Some(Ok(Message::Ping(_)|Message::Pong(_)))=>continue,_=>break};
                if text.len()>64*1024{break;}
                let request=match serde_json::from_str::<Request>(&text){Ok(request)=>request,Err(_)=>{let _=socket.send(Message::Text(serde_json::json!({"type":"error","message":"Invalid Files watch request"}).to_string().into())).await;break;}};
                let watched=match resolve(request.clone(),access.clone()).await{Ok(value)=>value,Err(message)=>{let _=socket.send(Message::Text(serde_json::json!({"type":"error","message":message}).to_string().into())).await;break;}};
                let (old,paths,dirty,signal)=(watcher.take(),watched.clone(),dirty.clone(),signal.clone());
                match tokio::task::spawn_blocking(move||install(old,paths,dirty,signal)).await{
                    Ok(Ok(active))=>watcher=Some(active),
                    Ok(Err(error))=>{let _=socket.send(Message::Text(serde_json::json!({"type":"error","message":format!("Filesystem watch unavailable: {error}; use Refresh")}).to_string().into())).await;break;},
                    _=>{let _=socket.send(Message::Text(serde_json::json!({"type":"error","message":"Filesystem watch setup failed; use Refresh"}).to_string().into())).await;break;},
                }
                let paths=watched.iter().map(|(relative,_)|relative).collect::<Vec<_>>();
                if socket.send(Message::Text(serde_json::json!({"type":"ready","paths":paths}).to_string().into())).await.is_err(){break;}
                subscription=Some((request,watched));
            }
            changed=changes.recv()=>{
                if changed.is_none(){break;}
                tokio::time::sleep(Duration::from_millis(150)).await;
                while changes.try_recv().is_ok(){}
                let paths=dirty.lock().map(|mut d|std::mem::take(&mut *d)).unwrap_or_default();
                if paths.is_empty(){continue;}
                // Revalidate configured shared roots and symlink targets before
                // disclosing activity. Revocation never emits a stale change.
                if access.is_some(){
                    if let Some((request,watched))=&subscription{
                        if !resolve(request.clone(),access.clone()).await.is_ok_and(|current|current==*watched){
                            let _=socket.send(Message::Text(serde_json::json!({"type":"error","message":"Files access changed; refresh the tree"}).to_string().into())).await;break;
                        }
                    }
                }
                if socket.send(Message::Text(serde_json::json!({"type":"changed","paths":paths}).to_string().into())).await.is_err(){break;}
            }
        }
    }
    let _ = tokio::task::spawn_blocking(move || drop(watcher)).await;
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
