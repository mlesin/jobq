use crate::Job;
use anyhow::{anyhow, Error};
use async_channel::Receiver;
use std::fmt::Debug;
use tokio::{process::Command, sync::mpsc::UnboundedSender};
use tokio_util::sync::CancellationToken;
use tracing::*;
use uuid::Uuid;

#[derive(Debug)]
pub enum WorkMessage {
    JobCompleted(Uuid),
    JobFailed(Uuid, String),
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
                        error!(message="Error receiving job by worker, exitting", error=?err);
                        break;
                    },
                    Ok(job) => {
                        // In case when we can't send response, there is no one to be notified about that, so just panic
                        let job_id = job.id;
                        match process(job).await {
                            Ok(()) => {
                                send_to_server.send(WorkMessage::JobCompleted(job_id)).unwrap();
                            },
                            Err(err) => {
                                send_to_server.send(WorkMessage::JobFailed(job_id, err.to_string())).unwrap();
                            }
                        };
                    }

                }
            }

        }
    }
    info!("Worker stopped.");
}

#[instrument(skip(job), fields(job_id = %job.id))]
async fn process(job: Job) -> Result<(), Error> {
    let cmd = vec![
        "/bin/bash",
        "-c",
        r#"number=$RANDOM; let "number%=6"; [ "$number" -ne 0 ] && sleep $number && echo success || (echo failure 1>&2; exit 1)"#,
    ];
    let cwd = ".";

    if let Some(program) = cmd.first() {
        debug!("Task \"{}\": Starting command \"{}\"", job.id, program);
        let output = Command::new(program)
            .args(&cmd[1..cmd.len()])
            .current_dir(cwd)
            .kill_on_drop(true)
            .output()
            .await?;
        if output.status.success() {
            return Ok(());
        } else {
            return Err(anyhow!("{}", String::from_utf8_lossy(&output.stderr)));
        }
    } else {
        return Err(anyhow!("No command specified"));
    }
}
