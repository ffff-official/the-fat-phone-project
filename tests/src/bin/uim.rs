use tests::*;

use qmi_sim_manager::ClientHandle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let _dest = env::var("DEST").unwrap();

    let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    let client = client_handle.client();

    let result = client
        .with_uim(async move |client, _tk| client.get_card_status().await)
        .await?;
    dbg!(result)?;

    let result = client
        .with_uim(async move |client, _tk| client.verify_pin("1234").await)
        .await?;
    dbg!(result)?;

    let result = client
        .with_uim(async move |client, _tk| client.get_card_status().await)
        .await?;
    dbg!(result)?;

    client_handle.shutdown().await?;
    Ok(())
}
