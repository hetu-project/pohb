use std::fmt::Write;

use bytes::Bytes;
use pohb::{OrdinaryClock, StageSource, TaskResult, TaskStage};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use tokio_stream::StreamExt as _;
use tracing::info;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let input = b"hello"; //
    let task_id = rand::random();

    let mut event_source = EventSource::get("http://localhost:3000/chain");
    let Some(event) = event_source.next().await else {
        anyhow::bail!("empty event source")
    };
    let Event::Open = event? else {
        anyhow::bail!("unimplemented")
    };

    let task_stage = TaskStage::<OrdinaryClock, _> {
        id: task_id,
        source: StageSource::Start,
        input: Bytes::from(input.to_vec()),
        clocks: Default::default(),
    };
    Client::new()
        .post("http://localhost:3000/gossip/publish")
        .json(&task_stage)
        .send()
        .await?
        .error_for_status()?;

    while let Some(event) = event_source.next().await {
        let Event::Message(message) = event? else {
            anyhow::bail!("unimplemented")
        };
        let message = serde_json::from_str::<TaskResult<OrdinaryClock, Bytes>>(&message.data)?;
        if message.id != task_id {
            continue;
        }
        info!("task done");
        info!("clocks");
        for (stage, clock) in &message.clocks {
            info!("  {stage}: {clock:?}")
        }
        info!("output");
        let mut output_line = String::from("  ");
        for b in &message.output {
            write!(&mut output_line, "{b:02x} ")?
        }
        info!("{output_line}");
        return Ok(());
    }
    anyhow::bail!("event source exhausted before task finished")
}
