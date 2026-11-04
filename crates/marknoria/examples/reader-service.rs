//! Standalone embedded-reader smoke harness; no desktop dependencies.
#[tokio::main]
async fn main() {
    if let Some(result) = marknoria::server::index_worker() {
        if let Err(error) = result {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let source = std::env::args_os().nth(1).expect("source path");
    let service = marknoria::server::Service::start(source.into())
        .await
        .unwrap();
    println!(
        "{}",
        serde_json::json!({"address":service.address.to_string(),"token":service.token})
    );
    tokio::signal::ctrl_c().await.unwrap();
}
