use crate::{Job, ServerMessage, ToMpart, WorkerMessage};
use anyhow::{anyhow, Error};
use async_trait::async_trait;
use futures::{channel::mpsc::unbounded, future::ready, SinkExt, StreamExt};
use std::{fmt::Debug, time::Duration};
use tmq::{dealer, Context, Multipart};
use tokio::time::sleep;
use tracing::*;

#[async_trait]
pub trait Worker: Sized + Debug {
    const JOB_NAME: &'static str;

    async fn process(&self, job: Job) -> Result<(), Error>;

    // #[instrument(level = "info")]
    async fn work(&self, job_address: &str) -> Result<(), Error> {
        let job_type = Self::JOB_NAME;
        debug!("Worker `{}` starting, sending hello", job_type);

        let (mut send_skt, recv) = dealer(&Context::new())
            .set_identity(job_type.as_bytes())
            .connect(&job_address)?
            .split::<Multipart>();

        let (send, mut recv_skt) = unbounded::<ServerMessage>();

        tokio::spawn(async move {
            while let Some(jobq_message) = recv_skt.next().await {
                if let Ok(msg) = jobq_message.to_mpart() {
                    if let Err(err) = send_skt.send(msg).await {
                        error!("Error sending message:{}", err);
                    }
                }
            }
        });

        let mut ping_sender = send.clone();

        tokio::spawn(
            async move {
                loop {
                    debug!("Sending ping");
                    if let Err(err) = ping_sender.send(ServerMessage::Hello).await {
                        error!("Error:{}", err);
                    };
                    sleep(Duration::from_millis(10000)).await;
                }
            }, // .instrument(info_span!("ping_sender")),
        );

        recv.filter_map(|val| {
            debug!(message = "Filtering", val=?val);
            match val
                .map_err(Error::from)
                .and_then(|msg| serde_cbor::from_slice(&msg[0]).map_err(Error::from))
            {
                Ok(WorkerMessage::Order(job)) => return ready(Some(job)),
                Ok(WorkerMessage::Hello) => {
                    debug!("Pong: {}", job_type);
                }
                Err(err) => {
                    error!("Error decoding message:{}", err);
                }
            }

            return ready(None);
        })
        .map(|job| (self.process(job.clone()), send.clone(), job))
        .for_each_concurrent(None, |(status, mut send, job)| async move {
            let server_message = match status.await {
                Ok(()) => ServerMessage::Completed(job),
                Err(err) => ServerMessage::Failed(job, err.to_string()),
            };

            debug!(message = ?server_message, "Sending server message");
            if let Err(err) = send.send(server_message).await {
                error!("Error sending server message: {}", err);
            }
        })
        .await;

        Ok(())
    }
}

#[derive(Debug)]
pub struct TestWorker;

#[async_trait]
impl Worker for TestWorker {
    const JOB_NAME: &'static str = "test";

    #[instrument(skip(job), fields(job_id = %job.id))]
    async fn process(&self, job: Job) -> Result<(), Error> {
        sleep(Duration::from_millis(100)).await;
        if job.id.as_u128() % 12 == 0 {
            return Err(anyhow!("Simulating failure"));
        }

        Ok(())
    }
}
