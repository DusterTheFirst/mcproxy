use std::{fmt, sync::Mutex};

use prometheus_client::{collector::Collector, encoding::DescriptorEncoder};
use tokio_metrics::{RuntimeIntervals, RuntimeMonitor};

use crate::metric;

#[derive(Debug)]
pub struct TokioRuntimeCollector {
    intervals: Mutex<RuntimeIntervals>,
}

impl TokioRuntimeCollector {
    pub fn new() -> Self {
        let handle = tokio::runtime::Handle::current();
        let runtime_monitor = RuntimeMonitor::new(&handle);

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
        let prefix = "tokio_runtime_";

        metric!(
            gauge in encoder for prefix,
            metrics.workers_count,
            "number of worker threads used by the runtime."
        );

        metric!(
            counter in encoder for prefix,
            metrics.total_park_count,
            "number of times worker threads parked."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_park_count,
            "maximum number of times any worker thread parked."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_park_count,
            "minimum number of times any worker thread parked."
        );

        metric!(
            gauge_duration in encoder for prefix,
            metrics.mean_poll_duration,
            "average duration of a single invocation of poll on a task."
        );
        metric!(
            gauge_duration in encoder for prefix,
            metrics.mean_poll_duration_worker_min,
            "average duration of a single invocation of poll on a task on the worker with the lowest value."
        );
        metric!(
            gauge_duration in encoder for prefix,
            metrics.mean_poll_duration_worker_max,
            "average duration of a single invocation of poll on a task on the worker with the highest value."
        );

        // TODO: FIXME: const histogram?
        // {
        //     Histogram::new(metrics.poll_count_histogram.into_iter().map(|x| x as f64));
        // }

        metric!(
            counter in encoder for prefix,
            metrics.total_noop_count,
            "number of times worker threads unparked but performed no work before parking again."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_noop_count,
            "maximum number of times any worker thread unparked but performed no work before parking again."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_noop_count,
            "minimum number of times any worker thread unparked but performed no work before parking again."
        );

        metric!(
            counter in encoder for prefix,
            metrics.total_steal_count,
            "number of tasks worker threads stole from another worker thread."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_steal_count,
            "maximum number of tasks any worker thread stole from another worker thread."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_steal_count,
            "minimum number of tasks any worker thread stole from another worker thread."
        );

        metric!(
            counter in encoder for prefix,
            metrics.total_steal_operations,
            "number of times worker threads stole tasks from another worker thread."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_steal_operations,
            "maximum number of times any worker thread stole tasks from another worker thread."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_steal_operations,
            "minimum number of times any worker thread stole tasks from another worker thread."
        );

        metric!(
            counter in encoder for prefix,
            metrics.num_remote_schedules,
            "number of tasks scheduled from **outside** of the runtime."
        );

        metric!(
            counter in encoder for prefix,
            metrics.total_local_schedule_count,
            "number of tasks scheduled from worker threads."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_local_schedule_count,
            "maximum number of tasks scheduled from any one worker thread."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_local_schedule_count,
            "minimum number of tasks scheduled from any one worker thread."
        );

        metric!(
            counter in encoder for prefix,
            metrics.total_overflow_count,
            "number of times worker threads saturated their local queues."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_overflow_count,
            "maximum number of times any one worker saturated its local queue."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_overflow_count,
            "minimum number of times any one worker saturated its local queue."
        );

        metric!(
            counter in encoder for prefix,
            metrics.total_polls_count,
            "number of tasks that have been polled across all worker threads."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_polls_count,
            "maximum number of tasks that have been polled in any worker thread."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_polls_count,
            "minimum number of tasks that have been polled in any worker thread."
        );

        metric!(
            counter_duration in encoder for prefix,
            metrics.total_busy_duration,
            "amount of time worker threads were busy."
        );
        metric!(
            gauge_duration in encoder for prefix,
            metrics.max_busy_duration,
            "maximum amount of time a worker thread was busy."
        );
        metric!(
            gauge_duration in encoder for prefix,
            metrics.min_busy_duration,
            "minimum amount of time a worker thread was busy."
        );

        metric!(
            counter in encoder for prefix,
            metrics.injection_queue_depth,
            "number of tasks currently scheduled in the runtime's injection queue."
        );
        metric!(
            counter in encoder for prefix,
            metrics.total_local_queue_depth,
            "total number of tasks currently scheduled in workers' local queues."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.max_local_queue_depth,
            "maximum number of tasks currently scheduled any worker's local queue."
        );
        metric!(
            gauge in encoder for prefix,
            metrics.min_local_queue_depth,
            "minimum number of tasks currently scheduled any worker's local queue."
        );

        metric!(
            gauge_duration in encoder for prefix,
            metrics.elapsed,
            "amount of time elapsed since observing runtime metrics."
        );

        metric!(
            counter in encoder for prefix,
            metrics.budget_forced_yield_count,
            "number of times that tasks have been forced to yield back to the scheduler after exhausting their task budgets."
        );

        metric!(
            counter in encoder for prefix,
            metrics.io_driver_ready_count,
            "number of ready events processed by the runtime's I/O driver."
        );

        Ok(())
    }
}
