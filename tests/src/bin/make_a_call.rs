use dotenvy::dotenv;
use futures::stream::StreamExt;
use qmi_sim_manager::ClientHandle;

use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let dest = env::var("DEST").unwrap();

    let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let mut events = client_handle.event_stream()?;
    let h = tokio::spawn(async move {
        // Process incoming events, such as received SMS messages or call events.
        while let Some(event) = events.next().await {
            println!("{event:?}");
        }
    });

    let client = client_handle.client();

    let call_id = client
        .with_voice(async move |client, _tk| client.dial_call(dest).await)
        .await??;

    tokio::time::sleep(Duration::from_secs(10)).await;

    let _call_id = client
        .with_voice(async move |client, _tk| client.end_call(call_id).await)
        .await??;

    tokio::time::sleep(Duration::from_secs(60)).await; // Keep the client running for one minute before shutting it down.
    client_handle.shutdown().await?;
    h.await?;
    Ok(())
}

fn configure_tracing() {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stdout)
        .with_filter(LevelFilter::DEBUG);

    drop(tracing_subscriber::registry().with(stdout_layer).try_init());
}
