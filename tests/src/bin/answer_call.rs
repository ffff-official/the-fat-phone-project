use dotenvy::dotenv;
use futures::stream::StreamExt;
use qmi_sim_manager::Event::VoiceCallEvent;
use qmi_sim_manager::libqmi::types::VoiceCallState;
use qmi_sim_manager::{Client, ClientHandle};

use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();

    let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    let mut events = client_handle.event_stream()?;
    let h = tokio::spawn(async move {
        // Process voice call events reported by the modem.
        while let Some(event) = events.next().await {
            if let VoiceCallEvent(items) = event {
                for item in items {
                    let client_clone = client.clone();
                    println!("{:#?}", item);
                    if item.state == VoiceCallState::Incoming {
                        dbg!("incoming call received");
                        if let Err(e) = tokio::spawn(answer(client_clone, item.id)).await {
                            eprintln!("Exiting with error : {e:?}");
                        }
                    }
                }
            }
        }
    });
    async fn answer(client: Client, id: u8) -> Result<(), qmi_sim_manager::Error> {
        client
            .with_voice(async move |client, _tk| client.answer_call(id).await)
            .await??;
        // Keep the call active for ten seconds.
        tokio::time::sleep(Duration::from_secs(10)).await;
        client
            .with_voice(async move |client, _tk| client.end_call(id).await)
            .await??;
        Ok(())
    }
    // Keep the client running for one minute before shutting it down.
    tokio::time::sleep(Duration::from_secs(60)).await;
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
