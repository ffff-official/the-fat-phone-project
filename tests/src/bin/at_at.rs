use futures::StreamExt;
use qmi_sim_manager::atat::{
    self, AtClientHandle,
    atat_derive::{AtatCmd, AtatResp},
};

use std::path::PathBuf;

#[derive(Clone, AtatCmd)]
#[at_cmd("+CSQ", CsqResponse, timeout_ms = 2000)]
struct At;

#[derive(Debug, Clone, AtatResp)]
pub struct CsqResponse {
    #[at_arg(position = 0)]
    pub rssi: u8,

    #[at_arg(position = 1)]
    pub ber: u8,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();

    let mut at_client_handle = AtClientHandle::init(PathBuf::from("/dev/ttyUSB3"), 115200)?;

    let at_client = at_client_handle.at_client();

    let mut at_events = at_client_handle.at_event_stream()?;
    let h = tokio::spawn(async move {
        // AT incoming URCs, such as RING\r
        while let Some(event) = at_events.next().await {
            println!("{event:?}");
        }
    });

    if let Err(e) = dbg!(at_client.send(At).await) {
        tracing::error!("Failed command : {:?}", e);
    }

    if let Err(e) = dbg!(at_client.send(At).await) {
        tracing::error!("Failed command : {:?}", e);
    }

    if let Err(e) = dbg!(at_client.send(At).await) {
        tracing::error!("Failed command : {:?}", e);
    }

    at_client_handle.shutdown().await?;
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
