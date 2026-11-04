//! Explicit retry before an API exists. The independent watch channel remains
//! available even if ordinary command queues fill during process launch.
use crate::{
    gateway::{GatewayLaunch, ManagedGateway},
    protocol::Event,
    startup::Update,
};
use tokio::sync::watch;
#[derive(Clone, Copy, Debug)]
pub enum Control {
    Attempt(u64),
    Close,
}
async fn publish(events: &async_channel::Sender<Event>, generation: u64, update: Update) -> bool {
    events
        .send(Event::Startup { generation, update })
        .await
        .is_ok()
}
pub async fn run(
    control: &mut watch::Receiver<Control>,
    events: &async_channel::Sender<Event>,
) -> Option<(ManagedGateway, u64)> {
    loop {
        let generation = match *control.borrow_and_update() {
            Control::Attempt(generation) => generation,
            Control::Close => return None,
        };
        if !publish(events, generation, Update::Launching).await {
            return None;
        }
        match GatewayLaunch::spawn() {
            Ok(mut launch) => {
                let result = tokio::select! {
                    result = launch.ready() => Some(result),
                    _ = control.changed() => None,
                };
                match result {
                    Some(Ok(api)) => {
                        let latest = *control.borrow();
                        match latest {
                            Control::Attempt(current) if current == generation => {
                                return Some((launch.finish(api), generation))
                            }
                            Control::Attempt(_) => {
                                launch.shutdown().await;
                                continue;
                            }
                            Control::Close => {
                                launch.shutdown().await;
                                return None;
                            }
                        }
                    }
                    Some(Err(error)) => {
                        launch.shutdown().await;
                        if !publish(events, generation, Update::LaunchFailed(error)).await {
                            return None;
                        }
                    }
                    None => {
                        launch.shutdown().await;
                        if control.has_changed().is_err() {
                            return None;
                        }
                        continue;
                    }
                }
            }
            Err(error) => {
                if !publish(events, generation, Update::LaunchFailed(error)).await {
                    return None;
                }
            }
        }
        if control.changed().await.is_err() {
            return None;
        }
    }
}
