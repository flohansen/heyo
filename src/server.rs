use chat::service::{heyo_chat::chat_server::ChatServer, HeyoChat};
use tonic::transport::Server;

pub mod chat;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap();

    let addr = "[::1]:50051".parse().unwrap();
    let chat_service = HeyoChat::new(jwt_secret);

    Server::builder()
        .add_service(ChatServer::new(chat_service))
        .serve(addr)
        .await?;

    Ok(())
}