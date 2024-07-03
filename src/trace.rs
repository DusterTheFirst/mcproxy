use tracing::level_filters::LevelFilter;

// Create a Resource that captures information about the entity for which telemetry is recorded.
#[cfg(feature = "telemetry")]
pub fn resource() -> opentelemetry_sdk::Resource {
    use opentelemetry_semantic_conventions::{
        resource::{DEPLOYMENT_ENVIRONMENT, SERVICE_NAME, SERVICE_VERSION},
        SCHEMA_URL,
    };

    opentelemetry_sdk::Resource::from_schema_url(
        [
            opentelemetry::KeyValue::new(SERVICE_NAME, env!("CARGO_PKG_NAME")),
            opentelemetry::KeyValue::new(SERVICE_VERSION, env!("CARGO_PKG_VERSION")),
            opentelemetry::KeyValue::new(DEPLOYMENT_ENVIRONMENT, "develop"),
        ],
        SCHEMA_URL,
    )
}

// Construct Tracer for OpenTelemetryLayer
#[cfg(feature = "telemetry")]
pub fn init_tracer() -> opentelemetry_sdk::trace::Tracer {
    opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                // Customize sampling strategy
                .with_sampler(opentelemetry_sdk::trace::Sampler::ParentBased(Box::new(
                    opentelemetry_sdk::trace::Sampler::TraceIdRatioBased(1.0),
                )))
                // If export trace to AWS X-Ray, you can use XrayIdGenerator
                .with_id_generator(opentelemetry_sdk::trace::RandomIdGenerator::default())
                .with_resource(resource()),
        )
        .with_batch_config(opentelemetry_sdk::trace::BatchConfig::default())
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .install_batch(opentelemetry_sdk::runtime::Tokio)
        .unwrap()
}

pub fn init_tracing_subscriber() {
    #[cfg(not(feature = "tokio-console"))]
    let console_layer = tracing_subscriber::layer::Identity::new();

    #[cfg(feature = "tokio-console")]
    let console_layer = console_subscriber::ConsoleLayer::builder()
        .with_default_env()
        .spawn();

    #[cfg(not(feature = "telemetry"))]
    let telemetry_layer = tracing_subscriber::layer::Identity::new();

    #[cfg(feature = "telemetry")]
    let telemetry_layer = tracing_opentelemetry::OpenTelemetryLayer::new(init_tracer())
        .with_filter(
            tracing_subscriber::filter::Targets::new()
                .with_target("mcproxy", tracing::level_filters::LevelFilter::TRACE),
        );

    use tracing_subscriber::{layer::SubscriberExt as _, util::SubscriberInitExt as _, Layer as _};

    tracing_subscriber::Registry::default()
        .with(tracing_error::ErrorLayer::default())
        .with(
            tracing_subscriber::fmt::layer().with_filter(
                tracing_subscriber::EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .from_env_lossy(),
            ),
        )
        .with(console_layer)
        .with(telemetry_layer)
        .init();
}
