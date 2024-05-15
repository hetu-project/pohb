use std::convert::identity;

use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use pohb::{OrdinaryClock, TaskResult, TaskStage};
use tokio::{net::TcpListener, sync::watch::Sender};
use tokio_stream::{wrappers::WatchStream, StreamExt as _};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let app = Router::new()
        .route("/gossip", get(gossip_subscribe))
        .route("/gossip/publish", post(gossip_publish))
        .with_state(Shared::new());
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

type C = OrdinaryClock;
type GossipMessage = TaskStage<C, Bytes>;
type ChainMessage = TaskResult<C, Bytes>;

#[derive(Clone)]
struct Shared {
    gossip: Sender<Option<GossipMessage>>,
    chain: Sender<Option<ChainMessage>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            gossip: Sender::new(None),
            chain: Sender::new(None),
        }
    }
}

impl Default for Shared {
    fn default() -> Self {
        Self::new()
    }
}

async fn gossip_subscribe(shared: State<Shared>) -> impl IntoResponse {
    let stream = WatchStream::new(shared.gossip.subscribe())
        .filter_map(identity)
        .map(|message| Event::default().json_data(message));
    Sse::new(stream)
}

async fn gossip_publish(shared: State<Shared>, Json(message): Json<GossipMessage>) {
    let _ = shared.gossip.send(Some(message));
}
