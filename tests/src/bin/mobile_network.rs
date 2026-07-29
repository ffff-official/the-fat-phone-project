use tests::*;

use qmi_sim_manager::{ClientHandle, libqmi::types::WdsConnectionStatus};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let _dest = env::var("DEST").unwrap();

    let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    let client = client_handle.client();

    let result = client
        .with_wds(async move |client, _tk| client.get_packet_service_status().await)
        .await?;
    assert_eq!(result?, WdsConnectionStatus::Disconnected);

    client.start_network("free").await?; // Activate a mobile data connection.

    let result = client
        .with_wds(async move |client, _tk| client.get_packet_service_status().await)
        .await?;
    assert_eq!(result?, WdsConnectionStatus::Connected);

    client.stop_network().await?;

    let result = client
        .with_wds(async move |client, _tk| client.get_packet_service_status().await)
        .await?;
    assert_eq!(result?, WdsConnectionStatus::Disconnected);

    client_handle.shutdown().await?;
    Ok(())
}
