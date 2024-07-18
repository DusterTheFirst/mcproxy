use config::schema::Config;
use connection::handle_connection;
use std::{ops::ControlFlow, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, task};
use trace::init_tracing_subscriber;
use tracing::{error, info, trace_span, Instrument};

use crate::proxy_server::ProxyServer;

mod config;
mod connection;
mod proto;
mod proxy_server;
mod trace;

#[cfg(feature = "discovery")]
mod discovery;
#[cfg(feature = "metrics")]
mod metrics;
#[cfg(feature = "pid1")]
mod signals;
#[cfg(feature = "ui")]
mod ui;

// TODO: FIXME: make better
include!(concat!(env!("OUT_DIR"), "/features.rs"));

#[tokio::main]
async fn main() -> eyre::Result<()> {
    #[cfg(feature = "pid1")]
    task::spawn(signals::handle_exit());

    init_tracing_subscriber();

    #[cfg(feature = "metrics")]
    let (registry, connection_metrics, active_connection_metrics) = metrics::create_metrics();

    #[cfg(feature = "autometrics")]
    let autometrics = autometrics::settings::AutometricsSettings::builder()
        .repo_url(env!("CARGO_PKG_REPOSITORY"))
        .service_name(env!("CARGO_PKG_NAME"))
        .prometheus_client_registry(registry)
        .init();

    #[cfg(feature = "autometrics")]
    let registry = autometrics.prometheus_client_registry();

    #[cfg(all(feature = "metrics", not(feature = "autometrics")))]
    let registry = Box::leak(Box::new(registry));

    info!(features=?ENABLED_FEATURES, "proxy starting");

    let mut args = std::env::args_os();
    let executable_name = args
        .next()
        .expect("first argument should be name of executable");
    let config_file = args
        .next()
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("./example/config.toml"));

    // TODO: command line options
    info!("loading config file");
    let initial_config: Arc<Config> = Arc::new(config::load(&config_file).await?);
    let (config_sender, config) = tokio::sync::watch::channel(initial_config.clone());
    // let config = task::spawn(config::watch(config_file));
    if let Some(config) = initial_config.ui {
        #[cfg(feature = "ui")]
        task::spawn(ui::listen(
            config,
            config_file,
            config_sender,
            #[cfg(feature = "metrics")]
            registry,
        ));

        #[cfg(not(feature = "ui"))]
        let _ = (config_sender, config);
    }

    #[cfg(feature = "discovery")]
    let discovered_servers = discovery::begin().await;

    let listener = TcpListener::bind(initial_config.proxy.listen_address)
        .await
        .expect("Unable to bind to socket");

    info!(
        listen_address = %initial_config.proxy.listen_address,
        "proxy server listening",
    );

    drop(initial_config);

    // Accept connections as they come in
    loop {
        let stream = listener.accept().await;

        match stream {
            Ok((mut client_stream, _address)) => {
                // Clone pointers to the address map and server responses
                let config = config.borrow().clone();
                #[cfg(feature = "discovery")]
                let discovered_servers = discovered_servers.clone(); // TODO:
                #[cfg(feature = "metrics")]
                let (connection_metrics, active_connection_metrics) = (
                    connection_metrics.clone(),
                    active_connection_metrics.clone(),
                );

                // Get the connection id
                let connection_id = client_stream.peer_addr().unwrap().port();

                // Fork off the connection handling
                let task = async move {
                    // Handle the connection
                    match handle_connection(
                        connection_id,
                        config,
                        &mut client_stream,
                        #[cfg(feature = "discovery")]
                        discovered_servers,
                        #[cfg(feature = "metrics")]
                        connection_metrics,
                    )
                    .await
                    {
                        Ok(ControlFlow::Continue((server_stream, upstream, handshake))) => {
                            #[cfg(feature = "metrics")]
                            let upstream = upstream.into();

                            #[cfg(feature = "metrics")]
                            active_connection_metrics
                                .active_server_connections
                                .get_or_create(&upstream)
                                .inc();

                            // Spin up constant proxy until the connection is complete
                            ProxyServer::new(server_stream, client_stream)
                                .start()
                                .instrument(trace_span!(
                                    "proxy",
                                    connection=connection_id,
                                    address = handshake.address,
                                    next_state = %handshake.next_state
                                ))
                                .await;

                            #[cfg(feature = "metrics")]
                            active_connection_metrics
                                .active_server_connections
                                .get_or_create(&upstream)
                                .dec();
                        }
                        Ok(ControlFlow::Break(())) => {}
                        Err(e) => {
                            error!("Error in handling connection: {}", e);
                        }
                    };
                }
                .instrument(trace_span!("connection"));

                #[cfg(feature = "tokio-console")]
                task::Builder::new()
                    .name(&connection_id.to_string())
                    .spawn(task)
                    .unwrap();

                #[cfg(not(feature = "tokio-console"))]
                task::spawn(task);
            }
            Err(e) => error!("Error connecting to client: {}", e),
        }
    }
}
