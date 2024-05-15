use std::{env::args, fs::canonicalize, process::Stdio};

use bytes::Bytes;
use pohb::{ClockContext, OrdinaryClock, OrdinaryContext, StageSource, TaskStage, Workflow};
use reqwest::Client;
use reqwest_eventsource::{Event, EventSource};
use tokio::{fs, io::AsyncWriteExt as _, process::Command};
use tokio_stream::StreamExt as _;
use tracing::{info, warn};

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let task = args()
        .nth(1)
        .ok_or(anyhow::format_err!("missing task description"))?;
    let task = serde_json::from_str::<Workflow>(&fs::read_to_string(task).await?)?;
    let stage = args()
        .nth(2)
        .ok_or(anyhow::format_err!("missing stage name"))?;
    let source = task
        .stages
        .iter()
        .take_while(|other_stage| **other_stage != stage)
        .last()
        .cloned()
        .map(StageSource::Name)
        .unwrap_or(StageSource::Start);

    let id = rand::random();
    let context = OrdinaryContext::<Bytes, _>::new(id);
    let mut event_source = EventSource::get("http://localhost:3000/gossip");
    while let Some(event) = event_source.next().await {
        let message = match event? {
            Event::Open => {
                info!("gossip initialized");
                continue;
            }
            Event::Message(message) => {
                serde_json::from_str::<TaskStage<OrdinaryClock, Bytes>>(&message.data)?
            }
        };
        if message.source != source {
            continue;
        }
        if let Err(err) = message.verify(&task, &context) {
            warn!("failed to verify gossip message: {err}");
            continue;
        }

        info!("start execute for task {:08x}", message.id);
        let mut child = Command::new(canonicalize(".")?.join("scripts").join(&stage))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&message.input)
            .await?;
        let output = child.wait_with_output().await?;
        anyhow::ensure!(output.status.success());
        let output = Bytes::from(output.stdout);

        let mut clocks = message.clocks;
        clocks.insert(
            stage.clone(),
            context.prove(
                &match &source {
                    StageSource::Start => Vec::new(),
                    StageSource::Name(name) => vec![(&clocks[name], &message.input)],
                },
                &output,
            )?,
        );
        let task_stage = TaskStage {
            id: message.id,
            source: StageSource::Name(stage.clone()),
            input: output,
            clocks,
        };
        Client::new()
            .post("http://localhost:3000/gossip/publish")
            .json(&task_stage)
            .send()
            .await?
            .error_for_status()?;
    }

    Ok(())
}
