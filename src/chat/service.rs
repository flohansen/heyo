pub mod heyo_chat {
    tonic::include_proto!("heyo_chat");
}

use std::{collections::HashMap, sync::Arc};

use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Status, Request, Response, Code};

use heyo_chat::{Message, JoinRequest, Empty, chat_server::Chat};

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
    connections: Arc<RwLock<Connections>>,
}

impl HeyoChat {
    pub fn new() -> HeyoChat {
        let users = HashMap::new();
        let connections = Arc::new(RwLock::new(Connections { users }));
        HeyoChat { connections }
    }
}

#[tonic::async_trait]
impl Chat for HeyoChat {
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