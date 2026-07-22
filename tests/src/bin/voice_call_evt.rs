use dotenvy::dotenv;
use futures::stream::StreamExt;
use qmi_sim_manager::SmClientHandle;

use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let _dest = env::var("DEST").unwrap();

    let mut client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    // Add a second custom listener
    let result = client
        .with_voice(|mut client, _tk| async move {
            client
                .set_all_call_status(Box::new(move |call_info_vec| {
                    for item in call_info_vec {
                        tracing::info!("mode = {:?}", item.info.mode);
                    }
                }))
                .await
                .unwrap();
            Ok::<(), ()>(())
        })
        .await?;

    dbg!(result.unwrap());

    let mut events = client_handle.event_stream()?;
    let h = tokio::spawn(async move {
        // Process incoming events, such as received CALL messages.
        while let Some(event) = events.next().await {
            println!("{event:?}");
        }
    });
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
