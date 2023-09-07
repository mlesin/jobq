use crate::Job;
use anyhow::{anyhow, Error};
use async_channel::Receiver;
use std::{fmt::Debug, time::Duration};
use tokio::{sync::mpsc::UnboundedSender, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::*;
use uuid::Uuid;

#[derive(Debug)]
pub enum WorkMessage {
    Started(Uuid),
    Completed(Uuid),
    Failed(Uuid, String),
}

// #[instrument(level = "info")]
pub async fn start(
    cancel_token: CancellationToken,
    recv_from_queue: Receiver<Job>,
    send_to_server: UnboundedSender<WorkMessage>,
) {
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                debug!("Worker Cancelled");
                break;
            },
            job = recv_from_queue.recv() => {
                match job {
                    Err(err) => {
                        error!("Error:{}", err);
                    },
                    Ok(job) => {
                        send_to_server.send(WorkMessage::Started(job.id)).unwrap();
                        let job_id = job.id;
                        match process(job).await {
                            Ok(()) => {
                                send_to_server.send(WorkMessage::Completed(job_id)).unwrap();
                            },
                            Err(err) => {
                                send_to_server.send(WorkMessage::Failed(job_id, err.to_string())).unwrap();
                            }
                        };
                    }

                }
            }

        }
    }
    debug!("Worker stopped.");
}

#[instrument(skip(job), fields(job_id = %job.id))]
async fn process(job: Job) -> Result<(), Error> {
    sleep(Duration::from_millis(100)).await;
    if job.id.as_u128() % 12 == 0 {
        return Err(anyhow!("Simulating failure"));
    }

    Ok(())
}
