use tests::*;

use qmi_sim_manager::ClientHandle;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let dest = env::var("DEST").unwrap();

    let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    client.send_sms(&dest, "This is bullshit !", true).await?;

    client_handle.shutdown().await?;
    Ok(())
}
