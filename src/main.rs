use anyhow::Error;
use clap::Parser;
use jobq::telemetry;
use std::env;
use tokio::{
    signal::unix::{signal, SignalKind},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info_span, instrument, Instrument};
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
        short = 'l',
        long = "listen_address",
        help = "Jobq Listen Address",
        default_value = "tcp://127.0.0.1:8888"
    )]
    job_address: String,
    #[arg(
        short = 'n',
        long = "number_active",
        help = "Number of Active Jobs in Parallel",
        default_value = "3"
    )]
    num: u16,
}

// #[instrument(skip_all, fields(message, job_id))]
// async fn try_get_message<S: TryStream<Ok = Multipart, Error = TmqError> + Unpin>(
//     recv: &mut S,
//     token: CancellationToken,
// ) -> Result<Option<ClientMessage>, Error> {
//     tokio::select! {
//         _ = token.cancelled() => {
//             debug!("Cancelled");
//             Ok(None)
//         },
//         r = recv.try_next() => {
//             if let Some(msg) = r? {
//                 let jobq_message: ClientMessage = serde_cbor::from_slice(&msg[0])?;
//                 match &jobq_message {
//                     ClientMessage::Hello => {
//                         tracing::Span::current().record("message", "Hello");
//                     },
//                     ClientMessage::Acknowledged(job) => {
//                         tracing::Span::current().record("message", "Acknowledged");
//                         tracing::Span::current().record("job_id", job.id.to_string());
//                     },
//                 };
//                 Ok(Some(jobq_message))
//             } else {
//                 Ok(None)
//             }
//         },
//     }
// }

#[instrument(skip(cancel_token))]
async fn setup(cancel_token: CancellationToken) -> Result<(), Error> {
    let config = ConfigContext::parse();

    let (send_to_server, recv_from_client) = mpsc::unbounded_channel();

    let server_spawn = {
        let cancel_token = cancel_token.clone();
        let connect_url = config.connect_url.clone();
        tokio::spawn(
            async move {
                if let Err(err) =
                    server::serve(cancel_token, connect_url, config.num, recv_from_client).await
                {
                    error!("Error starting server: {}", err);
                }
            }
            // .in_current_span(),
            .instrument(info_span!("server")),
        )
    };

    {
        for _ in 0..6 {
            let job_request = JobRequest {
                project_id: uuid!("12341234-1234-1234-1234-123412341234"),
                post_id: uuid!("43214321-4321-4321-4321-432143214321"),
                filename: "test.jpg".into(),
                hash: "1234567890ABCDEF1234567890ABCDEF".into(),
                mimetype: "image/jpeg".into(),
                sort_order: 1,
            };

            send_to_server.send(job_request)?;
        }

        debug!("Done!");

        // {
        //     // let span = info_span!("recv");
        //     // let _ = span.enter();
        //     while let Some(message) = try_get_message(&mut recv, cancel_token.clone()).await? {
        //         let msg = match message {
        //             ClientMessage::Acknowledged(job) => format!("Acknowledged({})", job.id),
        //             ClientMessage::Hello => "Hello".to_string(),
        //         };
        //         event!(Level::DEBUG, event = "Message", msg = ?msg);
        //     }
        // }
    }

    println!("Waiting for server to shutdown...");
    server_spawn.await?;

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
