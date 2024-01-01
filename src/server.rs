use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status, Code};

use heyo_chat::chat_service_server::ChatService;
use heyo_chat::{JoinRequest, Message, Empty};

use self::heyo_chat::chat_service_server::ChatServiceServer;

pub mod heyo_chat {
    tonic::include_proto!("heyo_chat");
}

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

pub struct HeyoChatService {
    connections: Arc<RwLock<Connections>>,
}

impl HeyoChatService {
    pub fn new() -> HeyoChatService {
        let users = HashMap::new();
        let connections = Arc::new(RwLock::new(Connections { users }));
        HeyoChatService { connections }
    }
}

#[tonic::async_trait]
impl ChatService for HeyoChatService {
    type JoinStream = ReceiverStream<Result<Message, Status>>;

    async fn join(
        &self,
        request: Request<JoinRequest>,
    ) -> Result<Response<Self::JoinStream>, Status> {
        let username = request.into_inner().username;
        let (stream_tx, stream_rx) = mpsc::channel(1);

        if let Some(_) = self.connections.read().await.users.get(&username) {
            return Err(Status::new(Code::AlreadyExists, "already connected"));
        }

        let (tx, mut rx) = mpsc::channel(1);

        self.connections
            .write()
            .await
            .users
            .insert(username, tx);

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse().unwrap();
    let chat_service = HeyoChatService::new();

    Server::builder()
        .add_service(ChatServiceServer::new(chat_service))
        .serve(addr)
        .await?;

    Ok(())
}