//! Serve reusable UI assets without opening or granting any document.
use axum::{Router, routing::get};
use std::{future::IntoFuture, io};
fn main() -> io::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let router = Router::new()
            .route(
                "/",
                get(|| async {
                    axum::response::Html(
                        "<!doctype html><html><body><h1>Public component host</h1></body></html>",
                    )
                }),
            )
            .nest("/reader-assets", marknoria::web::reader_assets());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = stopped.await;
                })
                .into_future(),
        );
        println!("http://{address}/");
        tokio::task::spawn_blocking(|| {
            let mut line = String::new();
            io::stdin().read_line(&mut line)
        })
        .await
        .map_err(io::Error::other)??;
        let _ = stop.send(());
        task.await.map_err(io::Error::other)?
    })
}
