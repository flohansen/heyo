pub mod heyo_chat {
    tonic::include_proto!("heyo_chat");
}

pub mod chat;

use std::io::BufRead;
use chat::jwt::Claims;
use chrono::{Utc, Duration};
use heyo_chat::{chat_client::ChatClient, JoinRequest, Message};
use jsonwebtoken::{Header, EncodingKey};
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let jwt_secret = std::env::var("JWT_SECRET").unwrap();

    let mut client = ChatClient::connect("http://[::1]:50051").await?;

    let stdin = std::io::stdin();

    let mut username = String::new();
    stdin.lock().read_line(&mut username)?;
    username = username.trim().to_string();

    let expiration = Utc::now()
        .checked_add_signed(Duration::minutes(30))
        .unwrap()
        .timestamp();

    let claims = Claims { username: username.clone(), exp: expiration };
    let key = EncodingKey::from_secret(jwt_secret.as_bytes());
    let token = jsonwebtoken::encode(&Header::default(), &claims, &key).unwrap();

    let join_request = JoinRequest { token: token };

    let mut stream = client
        .join(Request::new(join_request))
        .await?
        .into_inner();

    tokio::spawn(async move {
        while let Some(msg) = stream.message().await.unwrap() {
            println!("[{}] {}", msg.sender, msg.body);
        }
    });

    loop {
        let mut buffer = String::new();
        stdin.lock().read_line(&mut buffer)?;
        buffer = buffer.trim().to_string();

        let msg = Message { sender: username.clone(), body: buffer };
        client.send_message(msg).await?;
    }
}