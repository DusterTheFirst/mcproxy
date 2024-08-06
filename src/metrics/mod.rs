use std::{fmt::Debug, sync::Arc};

use crate::{config::schema::Config, metrics::tokio_collector::runtime::TokioRuntimeCollector};
use mcproxy_model::Upstream;
use minecraft_collector::MinecraftCollector;
use prometheus_client::{
    encoding::EncodeLabelSet,
    metrics::{counter::Counter, family::Family, gauge::Gauge, info::Info},
    registry::Registry,
};
use tokio::sync::watch::Receiver;
use tokio_collector::task::TokioTaskCollector;
use tokio_metrics::TaskMonitor;

mod minecraft_collector;
mod tokio_collector;

/// These are the labels used for the `build_info` metric.
#[derive(EncodeLabelSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct BuildInfo {
    pub branch: &'static str,
    pub commit: &'static str,
    pub version: &'static str,
    pub service_name: &'static str,
    pub repo_url: &'static str,
}

#[derive(Default, Clone)]
pub struct ConnectionMetrics {
    pub client_connections: Counter,
    pub client_handshakes_received: Counter,
    pub connection_unknown_upstream: Counter,
    pub connection_can_not_reach_upstream: Family<Upstream, Counter>,
    pub connection_established: Family<Upstream, Counter>,
}

#[derive(Default, Clone)]
pub struct ActiveConnectionMetrics {
    pub active_server_connections: Family<Upstream, Gauge>,
}

pub fn create_metrics(
    config: Receiver<Arc<Config>>,
) -> (
    Registry,
    ConnectionMetrics,
    ActiveConnectionMetrics,
    TaskMonitor,
) {
    let mut registry = Registry::default();

    registry.register(
        "build",
        "Info metric for tracking software version and build details",
        Info::new(BuildInfo {
            branch: env!("VERGEN_GIT_BRANCH"),
            commit: env!("VERGEN_GIT_SHA"),
            version: env!("CARGO_PKG_VERSION"),
            service_name: env!("CARGO_PKG_NAME"),
            repo_url: env!("CARGO_PKG_REPOSITORY"),
        }),
    );

    let connection_metrics = ConnectionMetrics::default();
    registry.register(
        "client_connections",
        "amount of incoming connections from minecraft clients",
        connection_metrics.client_connections.clone(),
    );
    registry.register(
        "client_handshakes_received",
        "amount of handshakes received from minecraft clients",
        connection_metrics.client_handshakes_received.clone(),
    );
    registry.register(
        "connection_unknown_upstream",
        "amount of connections that were rejected due to an unknown upstream",
        connection_metrics.connection_unknown_upstream.clone(),
    );
    registry.register(
        "connection_can_not_reach_upstream",
        "amount of connections that were rejected due to an unreachable upstream",
        connection_metrics.connection_can_not_reach_upstream.clone(),
    );
    registry.register(
        "connection_established",
        "amount of connections that fully established to an upstream",
        connection_metrics.connection_established.clone(),
    );

    let active_connection_metrics = ActiveConnectionMetrics::default();
    registry.register(
        "active_server_connections",
        "amount of active outgoing connections to minecraft servers",
        active_connection_metrics.active_server_connections.clone(),
    );

    // Tokio Runtime Metrics
    registry.register_collector(Box::new(TokioRuntimeCollector::new()));

    let proxy_task_monitor = TaskMonitor::new();
    registry.register_collector(Box::new(TokioTaskCollector::new(
        "proxy",
        &proxy_task_monitor,
    )));

    // Minecraft Upstream Metrics
    registry.register_collector(Box::new(MinecraftCollector::new(
        tokio::runtime::Handle::current(),
        config,
    )));

    (
        registry,
        connection_metrics,
        active_connection_metrics,
        proxy_task_monitor,
    )
}

#[macro_export]
macro_rules! metric {
    ($ty:tt in $encoder:ident for $prefix:ident, $($metrics:ident)?.$field:ident, $help:literal) => {
        metric!($ty in $encoder for $prefix, $($metrics)?.$field, $help, None)
    };
    ($ty:tt in $encoder:ident for $prefix:ident use $labels:ident, $($metrics:ident)?.$field:ident, $help:literal) => {
        metric!($ty in $encoder for $prefix use $labels, $($metrics)?.$field, $help, None)
    };
    (@descriptor $type:ident => $encoder:ident, $prefix:ident, $field:ident, $help:literal, $unit:expr) => {
        $encoder.encode_descriptor(
            &[::core::convert::AsRef::<str>::as_ref(&$prefix), stringify!($field)].concat(),
            $help,
            $unit,
            ::prometheus_client::metrics::MetricType::$type,
        )?
    };
    (gauge in $encoder:ident for $prefix:ident $(use $labels:expr)?, $($metrics:ident)?.$field:ident, $help:literal, $unit:expr) => {
        metric!(@descriptor Gauge => $encoder, $prefix, $field, $help, $unit)
            $(.encode_family(&$labels)?)?
            .encode_gauge(&($($metrics.)?$field as f64))?
    };
    (gauge_duration in $encoder:ident for $prefix:ident $(use $labels:expr)?, $($metrics:ident)?.$field:ident, $help:literal, None) => {
        metric!(@descriptor Gauge => $encoder, $prefix, $field, $help, Some(&prometheus_client::registry::Unit::Seconds))
            $(.encode_family(&$labels)?)?
            .encode_gauge(&($($metrics.)?$field.as_secs_f64()))?
    };
    (counter in $encoder:ident for $prefix:ident $(use $labels:expr)?, $($metrics:ident)?.$field:ident, $help:literal, $unit:expr) => {
        metric!(@descriptor Counter => $encoder, $prefix, $field, $help, $unit)
            $(.encode_family(&$labels)?)?
            .encode_counter(&($($metrics.)?$field as f64), None::<&prometheus_client::metrics::exemplar::Exemplar<(), f64>>)?
    };
    (counter_duration in $encoder:ident for $prefix:ident $(use $labels:expr)?, $($metrics:ident)?.$field:ident, $help:literal, None) => {
        metric!(@descriptor Counter => $encoder, $prefix, $field, $help, Some(&prometheus_client::registry::Unit::Seconds))
            $(.encode_family(&$labels)?)?
            .encode_counter(
                &($($metrics.)?$field.as_secs_f64()),
                None::<&prometheus_client::metrics::exemplar::Exemplar<(), f64>>,
            )?
    };
}
