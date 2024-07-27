use std::{
    error::Error,
    fmt::Write,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    extract::State,
    http::{header, HeaderName, HeaderValue, StatusCode},
    routing::method_routing,
};
use tokio::{
    io::{self},
    net::TcpListener,
    sync::watch::{Receiver, Sender},
};
use tracing::{debug, info};
use tracing_error::{InstrumentError, TracedError};

use crate::config::{
    self,
    schema::{Config, UiServerConfig},
};

pub async fn listen(
    config: UiServerConfig,
    config_path: PathBuf,
    sender: Sender<Arc<Config>>,
    config_receiver: Receiver<Arc<Config>>,
    #[cfg(feature = "metrics")] registry: &'static prometheus_client::registry::Registry,
) -> Result<(), TracedError<io::Error>> {
    let router = axum::Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .route(
            "/-/reload",
            method_routing::post(config_reload).with_state((sender, Arc::from(config_path))),
        )
        .route(
            "/-/config",
            method_routing::get(print_config).with_state(config_receiver),
        );

    #[cfg(feature = "metrics")]
    let router = router.route(
        "/metrics",
        method_routing::get(
            |State::<&'static prometheus_client::registry::Registry>(registry)| async move {
                let mut output = String::new();
                match prometheus_client::encoding::text::encode(&mut output, registry) {
                    Ok(()) => Ok((
                        [(
                            header::CONTENT_TYPE,
                            header::HeaderValue::from_static("text/plain; version=0.0.4"),
                        )],
                        output,
                    )),
                    Err(error) => Err((StatusCode::INTERNAL_SERVER_ERROR, error.to_string())),
                }
            },
        )
        .with_state(registry),
    );

    let socket = TcpListener::bind(config.listen_address).await?;

    info!(listen_address = %config.listen_address, "UI running");
    axum::serve(socket, router)
        .await
        .map_err(InstrumentError::in_current_span)
}

#[axum::debug_handler]
async fn print_config(
    State(config): State<Receiver<Arc<Config>>>,
) -> (
    [(axum::http::HeaderName, axum::http::HeaderValue); 1],
    std::string::String,
) {
    (
        [(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("text/plain"),
        )],
        format!("{:#?}", *config.borrow()),
    )
}

#[tracing::instrument(skip_all)]
#[axum::debug_handler]
async fn config_reload(
    State((sender, config_path)): State<(Sender<Arc<Config>>, Arc<Path>)>,
) -> Result<(StatusCode, &'static str), (StatusCode, String)> {
    let new_config = config::load(&config_path).await.map_err(|error| {
        let mut response = String::from("Failed to reload configuration");

        writeln!(response, "{error}").unwrap();

        let mut source = &error as &(dyn Error + 'static);
        while let Some(error) = source.source() {
            writeln!(response, "{error}").unwrap();
            source = error;
        }

        (StatusCode::INTERNAL_SERVER_ERROR, response)
    })?;

    debug!("new configuration parsed");
    sender.send_replace(Arc::new(new_config));
    info!("new configuration loaded");

    Ok((StatusCode::OK, "Configuration reloaded successfully"))
}
