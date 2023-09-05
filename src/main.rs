use anyhow::Error;
use clap::Parser;
use futures::{SinkExt, StreamExt, TryStreamExt};
use jobq::telemetry;

use serde_json::Value;
use std::env;
use std::time::Duration;
use tokio::{
    join,
    signal::unix::{signal, SignalKind},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info_span, instrument, Instrument};
use uuid::Uuid;

use jobq::server::Server;
use jobq::worker::{TestWorker, Worker};
use jobq::{dealer::Dealer, ClientMessage, JobRequest, Priority, ServerMessage};

#[derive(Parser, Clone, Debug, PartialEq)]
#[command(author, version)]
pub struct ConfigContext {
    #[arg(
        short = 'c',
        long = "connect_url",
        help = "PostgreSQL Connection URL",
        default_value = "postgres://jobq:jobq@127.0.0.1"
    )]
    connect_url: String,

    #[arg(
        short = 'l',
        long = "listen_address",
        help = "Jobq Listen Address",
        default_value = "127.0.0.1:8888"
    )]
    job_address: String,
    #[arg(
        short = 'n',
        long = "number_active",
        help = "Number of Active Jobs in Parallel",
        default_value = "4"
    )]
    num: usize,
}

#[instrument(level = "info", name = "setup", skip(token))]
async fn setup(token: CancellationToken) -> Result<(), Error> {
    let config = ConfigContext::parse();

    let server = Server::new(
        config.connect_url.clone(),
        config.job_address.clone(),
        config.num,
    );

    let server_spawn = {
        let cloned_token = token.clone();
        tokio::spawn(
            async move {
                tokio::select! {
                    _ = cloned_token.cancelled() => {
                        debug!("Cancelled");
                    },
                    r = server.serve() => {
                        if let Err(err) = r {
                            error!("Error starting server: {}", err);
                        }
                    },
                }
            }
            .instrument(info_span!("server")),
        )
    };

    sleep(Duration::from_millis(500)).await;

    let worker_spawn = {
        let worker_config = config.clone();
        let cloned_token = token.clone();
        tokio::spawn(
            async move {
                tokio::select! {
                    _ = cloned_token.cancelled() => {
                        debug!("Cancelled");
                    },
                    r = TestWorker.work(&worker_config.job_address) => {
                        if let Err(err) = r {
                            error!("Error starting worker: {}", err);
                        }
                    },
                }
            }
            .instrument(info_span!("worker")),
        )
    };

    {
        let span = info_span!("dealer");
        let _ = span.enter();
        let (mut send, mut recv) = Dealer::new(&config.job_address).await?.split();

        //Send hello
        send.send(ServerMessage::Hello("Test Client".into()))
            .await?;

        debug!("Hello sent");

        if let Some(ClientMessage::Hello(_name)) = recv.try_next().await? {
            debug!("Received Hello response, sending a couple of jobs");

            for i in 0..20 {
                let priority = if i % 2 == 0 {
                    Priority::High
                } else {
                    Priority::Normal
                };

                let job = JobRequest {
                    name: "test".into(),
                    username: "test_client".into(),
                    params: Value::Null,
                    uuid: Uuid::new_v4(),
                    priority,
                };

                send.send(ServerMessage::Request(job)).await?;
            }

            debug!("Done!");
        }

        {
            let span = info_span!("recv");
            let _ = span.enter();
            while let Some(message) = recv.try_next().await? {
                debug!("Message:{:?}", message);
            }
        }
    }

    let (srv, wrk) = join!(server_spawn, worker_spawn);
    srv?;
    wrk?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "jobq=DEBUG");
    }

    telemetry::init()?;

    let token = CancellationToken::new();

    let cloned_token = token.clone();
    let app = tokio::spawn(setup(cloned_token));

    tokio::spawn(async move {
        let mut sigterm = signal(SignalKind::terminate()).unwrap();
        let mut sigint = signal(SignalKind::interrupt()).unwrap();
        tokio::select! {
            _ = sigterm.recv() => {println!("Received SIGTERM"); token.cancel()},
            _ = sigint.recv() => {println!("Received SIGINT"); token.cancel()},
        }
    });
    app.await??;
    println!("Shutting down.");
    telemetry::shutdown();

    Ok(())
}
