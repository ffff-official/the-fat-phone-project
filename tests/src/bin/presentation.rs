// xxd -l 145 -p data-test/mms.bin | tr -d '\n' | sed 's/../& /g'
// xxd -r -p <<'EOF' | dd of=mms2.bin count=1 conv=notrunc status=none
// EOF
// RUSTFLAGS="-Zsanitizer=address -Cforce-frame-pointers=yes" cargo test -Zbuild-std --target x86_64-unknown-linux-gnu
use dotenvy::dotenv;
use futures::stream::StreamExt;
use qmi_sim_manager::ClientHandle;
use qmi_sim_manager::mms::{MmsContent, MmsContentType};

use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let dest = env::var("DEST").unwrap();

    let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    let result = client
        .with_wds(async move |client, _tk| client.get_packet_service_status().await)
        .await?;

    dbg!(result?);

    let result = client
        .with_wds(async move |client, tk| {
            let _r = tk
                .spawn(async move {
                    let _v = tokio::fs::read("bibendum.jpg").await;
                })
                .await;

            client.close().await
        })
        .await?;

    drop(dbg!(result));

    client.send_sms(&dest, "Hello belgium !", true).await?;

    client.start_network("free").await?; // A mobile data connection is required to send an MMS through the MMSC.
    let response = client
        .send_mms(
            "http://mms.free.fr",
            vec![MmsContent {
                content: tokio::fs::read("bibendum.jpg").await?,
                content_type: Some(MmsContentType::ImageJpeg),
            }],
            None,
            &[&dest],
        )
        .await?;
    println!("{:?}", response);
    client.stop_network().await?;

    let mut events = client_handle.event_stream()?;
    let h = tokio::spawn(async move {
        // Process incoming events, such as received SMS messages.
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
