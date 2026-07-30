use futures::StreamExt;
use tests::*;

use qmi_sim_manager::{ClientHandle, Event};

use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let _dest = env::var("DEST").unwrap();

    let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    let client = client_handle.client();

    let result = client
        .with_dms(async move |client, _tk| client.get_capabilities().await)
        .await?;
    println!("{:#?}", result?);

    let mut events = client_handle.event_stream()?;
    let h = tokio::spawn(async move {
        // Process incoming events, such as received CALL messages.
        while let Some(event) = events.next().await {
            if let Event::NasEvent(value) = &event {
                println!("nas event : {:?}", value);
            } else if let Event::NasSignalInfo(value) = &event {
                println!("Other signal info : {:?}", value);
            } else {
            }
        }
    });

    tokio::time::sleep(Duration::from_secs(60)).await; // Keep the client running for one minute before shutting it down.
    client_handle.shutdown().await?;
    h.await?;
    Ok(())
}
