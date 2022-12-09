include!(concat!(env!("OUT_DIR"), "/pets_router.rs"));

use std::net::SocketAddr;

#[tokio::main]
pub async fn main() -> Result<(), String> {
 let socket_address: SocketAddr = std::env::var("SOCKET_ADDRESS").unwrap_or_else(|_| "127.0.0.1:8000".to_string()).parse().unwrap(); axum::Server::bind(&socket_address).serve(router().into_make_service()).await.map_err(|e| e.to_string())
}
