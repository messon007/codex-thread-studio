//! Shared private HTTP fixture for protocol and source-reading tests.
use crate::gateway::Api;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub(crate) async fn receive(socket: &mut tokio::net::TcpStream) -> String {
    let mut bytes = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        let n = socket.read(&mut chunk).await.unwrap();
        assert!(n > 0);
        bytes.extend_from_slice(&chunk[..n]);
        assert!(bytes.len() < 1024 * 1024);
        if let Some(header) = bytes.windows(4).position(|p| p == b"\r\n\r\n") {
            let headers = String::from_utf8_lossy(&bytes[..header]).to_ascii_lowercase();
            let length = headers
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .map_or(0, |s| s.parse::<usize>().unwrap());
            if bytes.len() >= header + 4 + length {
                return String::from_utf8(bytes).unwrap();
            }
        }
    }
}
pub(crate) async fn server(
    responses: Vec<(u16, Value)>,
) -> (Api, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api = Api::from_access_url(&format!(
        "http://{}/#token=0123456789abcdef0123456789abcdef",
        listener.local_addr().unwrap()
    ))
    .unwrap();
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for (status, value) in responses {
            let (mut socket, _) = listener.accept().await.unwrap();
            requests.push(receive(&mut socket).await);
            let body = serde_json::to_vec(&value).unwrap();
            let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
            socket.write_all(header.as_bytes()).await.unwrap();
            socket.write_all(&body).await.unwrap();
        }
        requests
    });
    (api, task)
}
