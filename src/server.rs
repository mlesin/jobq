use crate::worker::{self, WorkMessage};
use crate::JobRequest;
use crate::{db::DbHandle, Job};
use anyhow::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::*;
use uuid::Uuid;

#[derive(Debug)]
pub enum WorkResponseMessage {
    JobCompleted(Uuid),
    JobFailed(Uuid, String),
    DatabaseQueueError(Error),
}

// #[instrument(level = "info")]
pub async fn serve(
    cancel_token: CancellationToken,
    connect_url: String,
    workers_count: u16,
    mut recv_from_client: mpsc::UnboundedReceiver<JobRequest>,
    send_to_client: mpsc::UnboundedSender<WorkResponseMessage>,
) -> Result<(), Error> {
    trace!("Connecting to db:{}", connect_url);
    let handle = DbHandle::new(&connect_url).await?;

    let (send_to_server, mut recv_from_worker) = mpsc::unbounded_channel::<WorkMessage>();
    let (send_to_queue, recv_from_queue) = async_channel::bounded::<Job>(workers_count as usize);

    let mut workers = vec![];
    for _ in 0..workers_count {
        let send_to_server = send_to_server.clone();
        let cancel_token = cancel_token.clone();
        let recv_from_queue = recv_from_queue.clone();
        let join_handle = tokio::spawn(
            async move {
                worker::start(cancel_token, recv_from_queue, send_to_server).await;
            }
            .instrument(info_span!("worker")),
        );
        workers.push(join_handle);
    }

    let mut free_workers = workers_count as i64;

    // Resubmit processing jobs (reset processing status to queued)
    handle.reset_processing_jobs().await?;

    loop {
        if free_workers > 0 {
            let jobs_to_process = handle.get_queued_jobs(free_workers).await?;
            for job in jobs_to_process {
                send_to_queue.send(job).await?;
                free_workers -= 1;
            }
        }

        // Waiting for something to else to happen to continue...
        tokio::select! {
            // Handle cancellation
            _ = cancel_token.cancelled() => {
                debug!("Server Cancelled");
                break;
            },
            // Handle responses from workers
            chan_msg = recv_from_worker.recv() => {
                match chan_msg {
                    None => {
                        debug!("Worker channel closed unexpectedly, exiting");
                        cancel_token.cancel();
                        break;
                    },
                    Some(WorkMessage::JobStarted(job_id)) => {
                        debug!(message = "Starting job", job_id = ?job_id);
                        let result = handle.begin_job(job_id).await;
                        if let Err(err) = result {
                            let result = send_to_client.send(WorkResponseMessage::DatabaseQueueError(err));
                            if let Err(err) = result {
                                error!(message = "Failed to send response to client", error = ?err);
                                cancel_token.cancel();
                                break;
                            }
                        }
                    },
                    Some(WorkMessage::JobCompleted(job_id)) => {
                        debug!(message = "Completed job", job_id = ?job_id);
                        free_workers += 1;
                        let result = handle.complete_job(job_id).await;
                        if let Err(err) = result {
                            let result = send_to_client.send(WorkResponseMessage::DatabaseQueueError(err));
                            if let Err(err) = result {
                                error!(message = "Failed to send response to client", error = ?err);
                                cancel_token.cancel();
                                break;
                            }
                        }
                        let result = send_to_client.send(WorkResponseMessage::JobCompleted(job_id));
                        if let Err(err) = result {
                            error!(message = "Failed to send response to client", error = ?err);
                            cancel_token.cancel();
                            break;
                        }
                    },
                    Some(WorkMessage::JobFailed(job_id, error_msg)) => {
                        debug!(message = "Failed job", job_id = ?job_id, error = ?error_msg);
                        free_workers += 1;
                        let result = handle.fail_job(job_id, &error_msg).await;
                        if let Err(err) = result {
                            let result = send_to_client.send(WorkResponseMessage::DatabaseQueueError(err));
                            if let Err(err) = result {
                                error!(message = "Failed to send response to client", error = ?err);
                                cancel_token.cancel();
                                break;
                            }
                        }
                        let result = send_to_client.send(WorkResponseMessage::JobFailed(job_id, error_msg));
                        if let Err(err) = result {
                            error!(message = "Failed to send response to client", error = ?err);
                            cancel_token.cancel();
                            break;
                        }
                    },
                }
            },
            // Handle requests from clients
            chan_msg = recv_from_client.recv() => {
                match chan_msg {
                    None => {
                        debug!("Client channel closed unexpectedly, exiting");
                        cancel_token.cancel();
                        break;
                    },
                    Some(job_request) => {
                        debug!(message = "Requested job", job_request = ?job_request);
                        let result = handle.submit_job_request(&job_request).await;
                        if let Err(err) = result {
                            let result = send_to_client.send(WorkResponseMessage::DatabaseQueueError( err));
                            if let Err(err) = result {
                                error!(message = "Failed to send response to client", error = ?err);
                                cancel_token.cancel();
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // Wait for all workers to complete
    futures::future::join_all(workers)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    info!("Server stopped.");

    Ok(())
}
