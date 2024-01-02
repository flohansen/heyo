pub mod heyo_chat {
    tonic::include_proto!("heyo_chat");
}

use jsonwebtoken::Algorithm;
use jsonwebtoken::Validation;
use jsonwebtoken::DecodingKey;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Status, Request, Response, Code};
use heyo_chat::{Message, JoinRequest, Empty, chat_server::Chat};

use super::jwt::Claims;

struct Connections {
    users: HashMap<String, mpsc::Sender<Message>>,
}

impl Connections {
    async fn broadcast(&self, msg: Message) {
        for (username, tx) in &self.users {
            if let Err(err) = tx.send(msg.clone()).await {
                println!("[broadcast] could not send to '{}', {:?}: {}", username, msg, err);
            }
        }
    }
}

pub struct HeyoChat {
    jwt_secret: String,
    connections: Arc<RwLock<Connections>>,
}

impl HeyoChat {
    pub fn new(jwt_secret: String) -> HeyoChat {
        let users = HashMap::new();
        let connections = Arc::new(RwLock::new(Connections { users }));
        HeyoChat { jwt_secret, connections }
    }
}

#[tonic::async_trait]
impl Chat for HeyoChat {
    type JoinStream = ReceiverStream<Result<Message, Status>>;

    async fn join(
        &self,
        request: Request<JoinRequest>,
    ) -> Result<Response<Self::JoinStream>, Status> {
        let token_str = request.into_inner().token;

        let key = DecodingKey::from_secret(self.jwt_secret.as_bytes());
        let validation = Validation::new(Algorithm::HS256);
        let token = jsonwebtoken::decode::<Claims>(&token_str, &key, &validation).unwrap();

        let (stream_tx, stream_rx) = mpsc::channel(1);

        if let Some(_) = self.connections.read().await.users.get(&token.claims.username) {
            return Err(Status::new(Code::AlreadyExists, "already connected"));
        }

        let (tx, mut rx) = mpsc::channel(1);

        self.connections
            .write()
            .await
            .users
            .insert(token.claims.username, tx);

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(err) = stream_tx.send(Ok(msg)).await {
                    println!("could not send message: {}", err);
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(stream_rx)))
    }

    async fn send_message(
        &self,
        request: Request<Message>,
    ) -> Result<Response<Empty>, Status> {
        let req = request.into_inner();
        let msg = Message {
            sender: req.sender,
            body: req.body,
        };

        self.connections
            .read()
            .await
            .broadcast(msg)
            .await;

        Ok(Response::new(Empty {}))
    }
}