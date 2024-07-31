pub mod runtime;
pub mod task;

#[macro_export]
macro_rules! metric {
    ($ty:tt in $encoder:ident for $prefix:ident, $metrics:ident.$field:ident, $help:literal) => {
        metric!($ty in $encoder for $prefix, $metrics.$field, $help, None)
    };
    ($ty:tt in $encoder:ident for $prefix:ident use $labels:ident, $metrics:ident.$field:ident, $help:literal) => {
        metric!($ty in $encoder for $prefix use $labels, $metrics.$field, $help, None)
    };
    (@descriptor $type:ident => $encoder:ident, $prefix:ident, $field:ident, $help:literal, $unit:expr) => {
        $encoder.encode_descriptor(
            &[::core::convert::AsRef::<str>::as_ref(&$prefix), stringify!($field)].concat(),
            $help,
            $unit,
            ::prometheus_client::metrics::MetricType::$type,
        )?
    };
    (gauge in $encoder:ident for $prefix:ident $(use $labels:expr)?, $metrics:ident.$field:ident, $help:literal, $unit:expr) => {
        metric!(@descriptor Gauge => $encoder, $prefix, $field, $help, $unit)
            $(.encode_family(&$labels)?)?
            .encode_gauge(&($metrics.$field as f64))?;
    };
    (gauge_duration in $encoder:ident for $prefix:ident $(use $labels:expr)?, $metrics:ident.$field:ident, $help:literal, None) => {
        metric!(@descriptor Gauge => $encoder, $prefix, $field, $help, Some(&prometheus_client::registry::Unit::Other("nanoseconds".into())))
            $(.encode_family(&$labels)?)?
            .encode_gauge(&($metrics.$field.as_nanos() as f64))?;
    };
    (counter in $encoder:ident for $prefix:ident $(use $labels:expr)?, $metrics:ident.$field:ident, $help:literal, $unit:expr) => {
        metric!(@descriptor Counter => $encoder, $prefix, $field, $help, $unit)
            $(.encode_family(&$labels)?)?
            .encode_counter(&($metrics.$field as f64), None::<&prometheus_client::metrics::exemplar::Exemplar<(), f64>>)?;
    };
    (counter_duration in $encoder:ident for $prefix:ident $(use $labels:expr)?, $metrics:ident.$field:ident, $help:literal, None) => {
        metric!(@descriptor Counter => $encoder, $prefix, $field, $help, Some(&prometheus_client::registry::Unit::Other("nanoseconds".into())))
            $(.encode_family(&$labels)?)?
            .encode_counter(
                &($metrics.$field.as_nanos() as f64),
                None::<&prometheus_client::metrics::exemplar::Exemplar<(), f64>>,
            )?;
    };
}
