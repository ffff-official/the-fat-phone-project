use tests::*;

use futures::stream::StreamExt;
use qmi_sim_manager::{ClientHandle, voice::VoiceBurstDtmfInput};

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

    tokio::time::sleep(Duration::from_secs(30)).await;

    if let Err(e) = client
        .with_voice(async move |client, _tk| {
            client
                .dtmf_tones_request(VoiceBurstDtmfInput {
                    call_id,
                    digits: "1234".to_string(),
                })
                .await
        })
        .await?
    {
        tracing::error!("Cannot send dtmf : {:?}", e);
    }

    tokio::time::sleep(Duration::from_secs(10)).await;

    let _call_id = client
        .with_voice(async move |client, _tk| client.end_call(call_id).await)
        .await??;

    tokio::time::sleep(Duration::from_secs(60)).await; // Keep the client running for one minute before shutting it down.
    client_handle.shutdown().await?;
    h.await?;
    Ok(())
}
