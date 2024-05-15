use std::{convert::identity, env::args, sync::Arc};

use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use pohb::{chain, OrdinaryClientContext, OrdinaryClock, TaskResult, TaskStage, Workflow};
use reqwest::StatusCode;
use tokio::{fs, net::TcpListener, sync::watch::Sender};
use tokio_stream::{wrappers::WatchStream, StreamExt as _};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let task = args()
        .nth(1)
        .ok_or(anyhow::format_err!("missing task description"))?;
    let task = serde_json::from_str(&fs::read_to_string(task).await?)?;
    let app = Router::new()
        .route("/gossip", get(gossip_subscribe))
        .route("/gossip/publish", post(gossip_publish))
        .route("/chain", get(chain_subscribe))
        .route("/chain/propose", post(chain_propose))
        .with_state(Shared::new(task));
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
    task: Arc<Workflow>,
    context: Arc<OrdinaryClientContext<Bytes>>,
}

impl Shared {
    fn new(task: Workflow) -> Self {
        Self {
            gossip: Sender::new(None),
            chain: Sender::new(None),
            task: Arc::new(task),
            context: Arc::new(OrdinaryClientContext::new()),
        }
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

async fn chain_subscribe(shared: State<Shared>) -> impl IntoResponse {
    let stream = WatchStream::new(shared.chain.subscribe())
        .filter_map(identity)
        .map(|message| Event::default().json_data(message));
    Sse::new(stream)
}

async fn chain_propose(shared: State<Shared>, Json(message): Json<ChainMessage>) -> Response {
    if let Err(err) = chain::verify(&message, &shared.task, &*shared.context) {
        return (StatusCode::FORBIDDEN, err.to_string()).into_response();
    }
    let _ = shared.chain.send(Some(message));
    StatusCode::OK.into_response()
}
