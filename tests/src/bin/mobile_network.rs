use qmi_sim_manager::{SmClientHandle, libqmi::types::WdsConnectionStatus};
use std::path::PathBuf;

use dotenvy::dotenv;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let _dest = env::var("DEST").unwrap();

    let client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    let client = client_handle.client();

    let result = client
        .with_wds(|client, _tk| async move { client.get_packet_service_status().await })
        .await?;
    assert_eq!(result?, WdsConnectionStatus::Disconnected);

    client.start_network("free").await?; // Activate a mobile data connection.

    let result = client
        .with_wds(|client, _tk| async move { client.get_packet_service_status().await })
        .await?;
    assert_eq!(result?, WdsConnectionStatus::Connected);

    client.stop_network().await?;

    let result = client
        .with_wds(|client, _tk| async move { client.get_packet_service_status().await })
        .await?;
    assert_eq!(result?, WdsConnectionStatus::Disconnected);

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
