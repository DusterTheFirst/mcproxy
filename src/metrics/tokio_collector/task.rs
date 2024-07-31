use std::fmt;

use prometheus_client::{
    collector::Collector,
    encoding::{DescriptorEncoder, EncodeLabelSet},
};
use tokio_metrics::TaskMonitor;

use crate::metric;

#[derive(Debug)]
pub struct TokioTaskCollector {
    task_name: &'static str,
    monitor: TaskMonitor,
}

impl TokioTaskCollector {
    pub fn new(task_name: &'static str, monitor: &TaskMonitor) -> Self {
        Self {
            task_name,
            monitor: monitor.clone(),
        }
    }
}

#[derive(EncodeLabelSet)]
struct TokioTaskLabels {
    task_name: &'static str,
}

impl Collector for TokioTaskCollector {
    fn encode(&self, mut encoder: DescriptorEncoder) -> Result<(), fmt::Error> {
        let metrics = self.monitor.cumulative();
        let prefix = "tokio_task_";

        let labels = TokioTaskLabels {
            task_name: self.task_name,
        };

        metric!(
            gauge in encoder for prefix use labels,
            metrics.instrumented_count,
            "number of tasks instrumented."
        );

        metric!(
            gauge in encoder for prefix use labels,
            metrics.dropped_count,
            "number of tasks dropped."
        );

        metric!(
            gauge in encoder for prefix use labels,
            metrics.first_poll_count,
            "number of tasks polled for the first time."
        );

        metric!(
            gauge_duration in encoder for prefix use labels,
            metrics.total_first_poll_delay,
            "total duration elapsed between the instant tasks are instrumented, and the instant they are first polled."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_idled_count,
            "total number of times that tasks idled, waiting to be awoken."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_idle_duration,
            "total duration that tasks idled."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_scheduled_count,
            "total number of times that tasks were awoken (and then, presumably, scheduled for execution)."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_scheduled_duration,
            "total duration that tasks spent waiting to be polled after awakening."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_poll_count,
            "total number of times that tasks were polled."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_poll_duration,
            "total duration elapsed during polls."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_fast_poll_count,
            "total number of times that polling tasks completed swiftly."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_fast_poll_duration,
            "total duration of fast polls."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_slow_poll_count,
            "total number of times that polling tasks completed slowly."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_slow_poll_duration,
            "total duration of slow polls."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_short_delay_count,
            "total count of tasks with short scheduling delays."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_short_delay_duration,
            "total duration of tasks with short scheduling delays."
        );

        metric!(
            counter in encoder for prefix use labels,
            metrics.total_long_delay_count,
            "total count of tasks with long scheduling delays."
        );
        metric!(
            counter_duration in encoder for prefix use labels,
            metrics.total_long_delay_duration,
            "total number of times that a task had a long scheduling duration."
        );

        Ok(())
    }
}
