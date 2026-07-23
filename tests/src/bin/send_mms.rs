use dotenvy::dotenv;
use qmi_sim_manager::ClientHandle;
use qmi_sim_manager::mms::{MmsContent, MmsContentType};

use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let dest = env::var("DEST").unwrap();

    let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    // A mobile data connection is required to send an MMS through the MMSC.
    client.start_network("free").await?;
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

    client_handle.shutdown().await?;
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
