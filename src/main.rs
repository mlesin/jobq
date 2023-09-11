use anyhow::Error;
use clap::Parser;

use jobq::telemetry;
use std::env;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, info_span, instrument, Instrument};
use uuid::uuid;

use jobq::server;
use jobq::JobRequest;

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
        short = 'n',
        long = "number_active",
        help = "Number of Active Jobs in Parallel",
        default_value = "3"
    )]
    num: u16,
}

#[instrument(skip(cancel_token))]
async fn setup(cancel_token: CancellationToken) -> Result<(), Error> {
    let config = ConfigContext::parse();

    // Channel for sending requests to be processed
    let (send_to_server, recv_from_client) = mpsc::unbounded_channel();

    // Channel for getting response from server
    let (send_to_client, mut recv_from_server) = mpsc::unbounded_channel();

    let server_spawn = {
        let cancel_token = cancel_token.clone();
        let connect_url = config.connect_url.clone();
        let send_to_client = send_to_client.clone();
        tokio::spawn(
            async move {
                if let Err(err) = server::serve(
                    cancel_token,
                    connect_url,
                    config.num,
                    recv_from_client,
                    send_to_client,
                )
                .await
                {
                    error!("Error starting server: {}", err);
                }
            }
            // .in_current_span(),
            .instrument(info_span!("server")),
        )
    };

    // Simulating client requests

    for _ in 0..10 {
        let job_request = JobRequest {
            project_id: uuid!("12341234-1234-1234-1234-123412341234"),
            post_id: uuid!("43214321-4321-4321-4321-432143214321"),
            filename: "test.jpg".into(),
            hash: "1234567890ABCDEF1234567890ABCDEF".into(),
            mimetype: "image/jpeg".into(),
            sort_order: 1,
            source_file: "./test/input/test.jpg".into(),
            target_file: "./test/output/test.jpg".into(),
        };

        send_to_server.send(job_request)?;
    }

    debug!("Done setting tasks");
    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                debug!("Server Cancelled");
                break;
            },
            server_response = recv_from_server.recv() => {
                match server_response {
                    Some(msg) => {
                        // let msg = match message {
                        //     ClientMessage::Acknowledged(job) => format!("Acknowledged({})", job.id),
                        //     ClientMessage::Hello => "Hello".to_string(),
                        // };
                        info!(event = "Message", msg = ?msg);
                    },
                    None => {
                        debug!("Server connection closed unexpectedly, exiting");
                        break;
                    }
                }
            }
        }
    }

    println!("Waiting for server to shutdown...");
    server_spawn.await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Current dir: {:?}", env::current_dir()?);
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
