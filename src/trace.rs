use opentelemetry::KeyValue;
use opentelemetry_sdk::{
    runtime,
    trace::{BatchConfig, RandomIdGenerator, Sampler, Tracer},
    Resource,
};
use opentelemetry_semantic_conventions::{
    resource::{DEPLOYMENT_ENVIRONMENT, SERVICE_NAME, SERVICE_VERSION},
    SCHEMA_URL,
};
use tracing::level_filters::LevelFilter;
use tracing_error::ErrorLayer;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    filter::Targets, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

// Create a Resource that captures information about the entity for which telemetry is recorded.
pub fn resource() -> Resource {
    Resource::from_schema_url(
        [
            KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
            KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            KeyValue::new(DEPLOYMENT_ENVIRONMENT, "develop"),
        ],
        SCHEMA_URL,
    )
}

// Construct Tracer for OpenTelemetryLayer
pub fn init_tracer() -> Tracer {
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                // Customize sampling strategy
                .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                    1.0,
                ))))
                // If export trace to AWS X-Ray, you can use XrayIdGenerator
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource()),
        )
        .with_batch_config(BatchConfig::default())
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .install_batch(runtime::Tokio)
        .unwrap()
}

pub fn init_tracing_subscriber() {
    #[cfg(not(feature = "tokio-console"))]
    let console_layer = tracing_subscriber::layer::Identity::new();

    #[cfg(feature = "tokio-console")]
    let console_layer = console_subscriber::ConsoleLayer::builder()
        .with_default_env()
        .spawn();

    tracing_subscriber::Registry::default()
        .with(ErrorLayer::default())
        .with(tracing_subscriber::fmt::layer().with_filter(EnvFilter::from_default_env()))
        .with(console_layer)
        .with(
            OpenTelemetryLayer::new(init_tracer())
                .with_filter(Targets::new().with_target("mcproxy", LevelFilter::TRACE)),
        )
        .init();
}
