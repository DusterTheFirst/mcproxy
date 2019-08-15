use mcproxy::{AsleepServer, Motd, SleepMode, KickMessage, ServerMode};
use std::thread;

fn main() {
    // TODO: Keep MOTD and icon of server, just change the version and the players
    let sleep_server = AsleepServer::new(ServerMode::Asleep {
        motd: Motd::Raw("The normal motd of the server".to_owned()),
        favicon: None,
        sleep_mode: SleepMode::WakeOnConnect,
        kick_msg: KickMessage::Default
    }, 25565);
    
    let sleep_server_thread = thread::spawn(move || {
        sleep_server.listen_until_wake();

        println!("25565 Awoken");
    });
    
    let offline_server = AsleepServer::new(ServerMode::Offline {
        motd: Motd::Raw("The normal motd of the server".to_owned()),
        favicon: None,
        kick_msg: KickMessage::Default
    }, 25566);

    let offline_server_thread = thread::spawn(move || {
        offline_server.listen_until_wake();

        println!("25566 Awoken");
    });

    sleep_server_thread.join().unwrap();
    offline_server_thread.join().unwrap();
}