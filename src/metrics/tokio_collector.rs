use std::{fmt, sync::Mutex};

use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeMetric},
    metrics::{counter::ConstCounter, gauge::ConstGauge},
    registry::Unit,
};
use tokio_metrics::{RuntimeIntervals, RuntimeMonitor};

#[derive(Debug)]
pub struct TokioRuntimeCollector {
    intervals: Mutex<RuntimeIntervals>,
}

impl TokioRuntimeCollector {
    pub fn new() -> Self {
        let handle = tokio::runtime::Handle::current();
        let runtime_monitor = tokio_metrics::RuntimeMonitor::new(&handle);

        TokioRuntimeCollector {
            intervals: Mutex::new(runtime_monitor.intervals()),
        }
    }
}

impl Collector for TokioRuntimeCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), fmt::Error> {
        let Some(metrics) = self.intervals.lock().unwrap().next() else {
            return Ok(());
        };

        macro_rules! metric {
            ($ty:ty, $field:ident, $help:literal) => {
                metric!($ty, $field, $help, None)
            };
            ($ty:ty, $field:ident, $help:literal, $unit:ident) => {{
                let metric = <$ty>::new(metrics.$field as _);
                metric.encode(encoder.encode_descriptor(
                    concat!("tokio_runtime_", stringify!($field)),
                    $help,
                    $unit,
                    metric.metric_type(),
                )?)?;
            }};
        }
        macro_rules! metric_duration {
            ($ty:ty, $field:ident, $help:literal) => {{
                let metric = <$ty>::new(metrics.$field.as_nanos() as f64);
                metric.encode(encoder.encode_descriptor(
                    concat!("tokio_runtime_", stringify!($field)),
                    $help,
                    Some(&Unit::Other("nanoseconds".into())),
                    metric.metric_type(),
                )?)?;
            }};
        }

        metric!(
            ConstGauge,
            workers_count,
            "number of worker threads used by the runtime."
        );

        metric!(
            ConstCounter,
            total_park_count,
            "number of times worker threads parked."
        );
        metric!(
            ConstGauge,
            max_park_count,
            "maximum number of times any worker thread parked."
        );
        metric!(
            ConstGauge,
            min_park_count,
            "minimum number of times any worker thread parked."
        );

        metric_duration!(
            ConstGauge<f64>,
            mean_poll_duration,
            "average duration of a single invocation of poll on a task."
        );
        metric_duration!(
            ConstGauge<f64>,
            mean_poll_duration_worker_min,
            "average duration of a single invocation of poll on a task on the worker with the lowest value."
        );
        metric_duration!(
            ConstGauge<f64>,
            mean_poll_duration_worker_max,
            "average duration of a single invocation of poll on a task on the worker with the highest value."
        );

        // TODO: FIXME: const histogram?
        // {
        //     Histogram::new(metrics.poll_count_histogram.into_iter().map(|x| x as f64));
        // }

        metric!(
            ConstCounter,
            total_noop_count,
            "number of times worker threads unparked but performed no work before parking again."
        );
        metric!(
            ConstGauge,
            max_noop_count,
            "maximum number of times any worker thread unparked but performed no work before parking again."
        );
        metric!(
            ConstGauge,
            min_noop_count,
            "minimum number of times any worker thread unparked but performed no work before parking again."
        );

        metric!(
            ConstCounter,
            total_steal_count,
            "number of tasks worker threads stole from another worker thread."
        );
        metric!(
            ConstGauge,
            max_steal_count,
            "maximum number of tasks any worker thread stole from another worker thread."
        );
        metric!(
            ConstGauge,
            min_steal_count,
            "minimum number of tasks any worker thread stole from another worker thread."
        );

        metric!(
            ConstCounter,
            total_steal_operations,
            "number of times worker threads stole tasks from another worker thread."
        );
        metric!(
            ConstGauge,
            max_steal_operations,
            "maximum number of times any worker thread stole tasks from another worker thread."
        );
        metric!(
            ConstGauge,
            min_steal_operations,
            "minimum number of times any worker thread stole tasks from another worker thread."
        );

        metric!(
            ConstCounter,
            num_remote_schedules,
            "number of tasks scheduled from **outside** of the runtime."
        );

        metric!(
            ConstCounter,
            total_local_schedule_count,
            "number of tasks scheduled from worker threads."
        );
        metric!(
            ConstGauge,
            max_local_schedule_count,
            "maximum number of tasks scheduled from any one worker thread."
        );
        metric!(
            ConstGauge,
            min_local_schedule_count,
            "minimum number of tasks scheduled from any one worker thread."
        );

        metric!(
            ConstCounter,
            total_overflow_count,
            "number of times worker threads saturated their local queues."
        );
        metric!(
            ConstGauge,
            max_overflow_count,
            "maximum number of times any one worker saturated its local queue."
        );
        metric!(
            ConstGauge,
            min_overflow_count,
            "minimum number of times any one worker saturated its local queue."
        );

        metric!(
            ConstCounter,
            total_polls_count,
            "number of tasks that have been polled across all worker threads."
        );
        metric!(
            ConstGauge,
            max_polls_count,
            "maximum number of tasks that have been polled in any worker thread."
        );
        metric!(
            ConstGauge,
            min_polls_count,
            "minimum number of tasks that have been polled in any worker thread."
        );

        metric_duration!(
            ConstCounter<f64>,
            total_busy_duration,
            "amount of time worker threads were busy."
        );
        metric_duration!(
            ConstGauge<f64>,
            max_busy_duration,
            "maximum amount of time a worker thread was busy."
        );
        metric_duration!(
            ConstGauge<f64>,
            min_busy_duration,
            "minimum amount of time a worker thread was busy."
        );

        metric!(
            ConstCounter,
            injection_queue_depth,
            "number of tasks currently scheduled in the runtime's injection queue."
        );
        metric!(
            ConstCounter,
            total_local_queue_depth,
            "total number of tasks currently scheduled in workers' local queues."
        );
        metric!(
            ConstGauge,
            max_local_queue_depth,
            "maximum number of tasks currently scheduled any worker's local queue."
        );
        metric!(
            ConstGauge,
            min_local_queue_depth,
            "minimum number of tasks currently scheduled any worker's local queue."
        );

        metric_duration!(
            ConstGauge<f64>,
            elapsed,
            "amount of time elapsed since observing runtime metrics."
        );

        metric!(
            ConstCounter,
            budget_forced_yield_count,
            "number of times that tasks have been forced to yield back to the scheduler after exhausting their task budgets."
        );

        metric!(
            ConstCounter,
            io_driver_ready_count,
            "number of ready events processed by the runtime's I/O driver."
        );

        Ok(())
    }
}
