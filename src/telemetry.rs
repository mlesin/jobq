use std::env;

use opentelemetry::global;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    env::remove_var("http_proxy");
    env::remove_var("https_proxy");
    env::remove_var("HTTP_PROXY");
    env::remove_var("HTTPS_PROXY");

    global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

    let tracer = opentelemetry_jaeger::new_collector_pipeline()
        .with_endpoint("http://jaeger:14268/api/traces")
        .with_service_name("jobq") // the name of the application
        .with_isahc() // requires `isahc_collector_client` feature
        .with_timeout(std::time::Duration::from_secs(2))
        .install_batch(opentelemetry::runtime::Tokio)?;

    // Create a tracing layer with the configured tracer
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    // The SubscriberExt and SubscriberInitExt traits are needed to extend the
    // Registry to accept `opentelemetry (the OpenTelemetryLayer type).
    tracing_subscriber::registry()
        .with(telemetry)
        // Continue logging to stdout
        .with(filter_layer)
        .with(fmt::Layer::default())
        .try_init()?;

    Ok(())
}

pub fn shutdown() {
    global::shutdown_tracer_provider();
}
