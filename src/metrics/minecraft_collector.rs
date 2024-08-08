use std::{
    collections::HashSet,
    sync::Arc,
    time::{Duration, Instant},
};

use mcproxy_model::Upstream;
use prometheus_client::collector::Collector;
use tokio::{net::TcpStream, runtime::Handle, sync::watch::Receiver, task::JoinSet};
use tracing::debug;

use crate::{
    config::schema::Config,
    proto::{io::request::server_list_ping, packet::response::StatusResponse},
};

#[derive(Debug)]
pub struct MinecraftCollector {
    tokio_runtime: Handle,

    // TODO: replace with only pointer to the upstream listing
    config: Receiver<Arc<Config>>,
}

impl MinecraftCollector {
    pub fn new(tokio_runtime: Handle, config: Receiver<Arc<Config>>) -> Self {
        MinecraftCollector {
            config,
            tokio_runtime,
        }
    }
}

impl Collector for MinecraftCollector {
    fn encode(
        &self,
        mut encoder: prometheus_client::encoding::DescriptorEncoder,
    ) -> Result<(), std::fmt::Error> {
        let mut futures = JoinSet::<(Upstream, Option<(Duration, StatusResponse)>)>::new();

        let upstreams: HashSet<_> = self
            .config
            .borrow()
            .static_servers
            .values()
            .cloned()
            .collect();

        for upstream in upstreams {
            futures.spawn(async move {
                let stream = match TcpStream::connect(upstream.addr()).await {
                    Ok(stream) => stream,
                    Err(error) => {
                        debug!(%upstream, %error, "failed to connect to upstream for ping");

                        return (upstream, None);
                    }
                };

                let (response_time, status_response) =
                    match server_list_ping(stream, upstream.clone()).await {
                        Ok(response) => response,
                        Err(error) => {
                            debug!(%upstream, %error, "failed to ping upstream");

                            return (upstream, None);
                        }
                    };

                (upstream, Some((response_time, status_response)))
            });
        }

        let scrape_start = Instant::now();
        self.tokio_runtime.block_on(async {
            let mut upstream_responses = Vec::with_capacity(futures.len());

            while let Some(result) = futures.join_next().await {
                let (upstream, metrics) = result.expect("join should not fail");

                upstream_responses.push((upstream, metrics));
            }

            let upstream_responses = upstream_responses; // Immutable;

            {
                let mut metric_encoder = encoder.encode_descriptor(
                    "mcproxy_upstream_healthy",
                    "if the server is healthy (1) or not (0)",
                    None,
                    prometheus_client::metrics::MetricType::Gauge,
                )?;

                for (upstream, metrics) in &upstream_responses {
                    let healthy = metrics.is_some();

                    metric_encoder
                        .encode_family(upstream)?
                        .encode_gauge(&u32::from(healthy))?;
                }
            }

            let healthy_upstream_responses =
                upstream_responses.iter().filter_map(|(upstream, metrics)| {
                    metrics.as_ref().map(|metrics| (upstream, metrics))
                });

            {
                let mut metric_encoder = encoder.encode_descriptor(
                    "mcproxy_upstream",
                    "metric for tracking upstream info",
                    None,
                    prometheus_client::metrics::MetricType::Info,
                )?;

                for (upstream, (_, status_response)) in healthy_upstream_responses.clone() {
                    // TODO: merge label set?
                    metric_encoder.encode_info(&[
                        ("minecraft_version", status_response.version.name.as_ref()),
                        ("host", upstream.host.as_ref()),
                        ("port", &upstream.port.to_string()),
                    ])?;
                }
            }

            {
                let mut metric_encoder = encoder.encode_descriptor(
                    "mcproxy_upstream_response_time",
                    "round trip time to to the server",
                    None,
                    prometheus_client::metrics::MetricType::Gauge,
                )?;

                for (upstream, (response_time, _)) in healthy_upstream_responses.clone() {
                    metric_encoder
                        .encode_family(upstream)?
                        .encode_gauge(&response_time.as_secs_f64())?;
                }
            }

            {
                let mut metric_encoder = encoder.encode_descriptor(
                    "mcproxy_upstream_players_online",
                    "current online players",
                    None,
                    prometheus_client::metrics::MetricType::Gauge,
                )?;

                for (upstream, (_, status_response)) in healthy_upstream_responses.clone() {
                    if let Some(players) = &status_response.players {
                        metric_encoder
                            .encode_family(upstream)?
                            .encode_gauge(&players.online)?;
                    }
                }
            }

            {
                let mut metric_encoder = encoder.encode_descriptor(
                    "mcproxy_upstream_players_max",
                    "configured maximum allowed players",
                    None,
                    prometheus_client::metrics::MetricType::Gauge,
                )?;

                for (upstream, (_, status_response)) in healthy_upstream_responses.clone() {
                    if let Some(players) = &status_response.players {
                        metric_encoder
                            .encode_family(upstream)?
                            .encode_gauge(&players.max)?;
                    }
                }
            }

            let duration = scrape_start.elapsed();
            encoder
                .encode_descriptor(
                    "mcproxy_upstream_collect_duration",
                    "length of time to process current scrape",
                    Some(&prometheus_client::registry::Unit::Seconds),
                    prometheus_client::metrics::MetricType::Gauge,
                )?
                .encode_gauge(&duration.as_secs_f64())?;

            Ok(())
        })
    }
}
