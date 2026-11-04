//! One owned, authenticated Files subscription. No filesystem work on GTK.
use crate::{gateway::Api, protocol::Event};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::time::Duration;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{protocol::WebSocketConfig, Message},
};
#[derive(Clone, Debug)]
pub struct Subscription {
    pub generation: u64,
    pub root: String,
    pub paths: Vec<String>,
}
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Update {
    Ready { paths: Vec<String> },
    Changed { paths: Vec<String> },
    Error { message: String },
}
pub async fn run(api: Api, subscription: Subscription, events: async_channel::Sender<Event>) {
    let generation = subscription.generation;
    if subscription.paths.len() > 128
        || subscription.paths.iter().any(|p| p.len() > 8192)
        || serde_json::to_vec(
            &serde_json::json!({"root":subscription.root,"paths":subscription.paths}),
        )
        .map_or(true, |v| v.len() > 64 * 1024)
    {
        let _ = events
            .send(Event::FilesWatch {
                generation,
                update: Update::Error {
                    message: "Files watch subscription exceeds its budget".into(),
                },
            })
            .await;
        return;
    }
    loop {
        let Err(message) = listen(api.clone(), subscription.clone(), &events).await else {
            return;
        };
        if events
            .send(Event::FilesWatch {
                generation,
                update: Update::Error { message },
            })
            .await
            .is_err()
        {
            return;
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}
async fn listen(
    api: Api,
    s: Subscription,
    events: &async_channel::Sender<Event>,
) -> Result<(), String> {
    let request = api.socket_request("/ws/native-files")?;
    let config = WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    let (mut socket, _) = tokio::time::timeout(
        Duration::from_secs(15),
        connect_async_with_config(request, Some(config), false),
    )
    .await
    .map_err(|_| "Files watch connection timed out")?
    .map_err(|e| e.to_string())?;
    socket
        .send(Message::text(
            serde_json::json!({"root":s.root,"paths":s.paths}).to_string(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    while let Some(message) = socket.next().await {
        match message.map_err(|e| e.to_string())? {
            Message::Text(text) => {
                let update = crate::worker::compute(move || {
                    let value: Update =
                        serde_json::from_str(&text).map_err(|_| "Invalid Files watch event")?;
                    match &value {
                        Update::Ready { paths } | Update::Changed { paths }
                            if paths.len() > 128 || paths.iter().any(|p| p.len() > 8192) =>
                        {
                            Err("Files watch event exceeds its budget".into())
                        }
                        _ => Ok(value),
                    }
                })
                .await?;
                let failed = matches!(update, Update::Error { .. });
                events
                    .send(Event::FilesWatch {
                        generation: s.generation,
                        update,
                    })
                    .await
                    .map_err(|_| "Native application closed")?;
                if failed {
                    return Ok(());
                }
            }
            Message::Ping(data) => socket
                .send(Message::Pong(data))
                .await
                .map_err(|e| e.to_string())?,
            Message::Close(_) => break,
            _ => {}
        }
    }
    Err("Live file updates disconnected; use Refresh".into())
}
