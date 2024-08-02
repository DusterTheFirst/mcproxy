mod discovery;

#[tokio::main]
async fn main() {
    #[cfg(feature = "discovery")]
    let discovered_servers = discovery::begin().await;

    // Provide configuration to main process somehow... Probably through a channel or port
    todo!();
}
