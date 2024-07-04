use tracing::info;

pub async fn handle_exit() -> ! {
    use tokio::signal::unix::{signal, SignalKind};

    let mut alarm = signal(SignalKind::alarm()).unwrap();
    let mut hangup = signal(SignalKind::hangup()).unwrap();
    let mut interrupt = signal(SignalKind::interrupt()).unwrap();
    let mut pipe = signal(SignalKind::pipe()).unwrap();
    let mut quit = signal(SignalKind::quit()).unwrap();
    let mut terminate = signal(SignalKind::terminate()).unwrap();
    let mut user_defined1 = signal(SignalKind::user_defined1()).unwrap();
    let mut user_defined2 = signal(SignalKind::user_defined2()).unwrap();

    tokio::select! {
        _ = alarm.recv() => info!("alarm"),
        _ = hangup.recv() => info!("hangup"),
        _ = interrupt.recv() => info!("interrupt"),
        _ = pipe.recv() => info!("pipe"),
        _ = quit.recv() => info!("quit"),
        _ = terminate.recv() => info!("terminate"),
        _ = user_defined1.recv() => info!("user_defined1"),
        _ = user_defined2.recv() => info!("user_defined2"),
    }

    std::process::exit(0);
}
