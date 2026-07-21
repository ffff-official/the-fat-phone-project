use dotenvy::dotenv;
use futures::stream::StreamExt;
use qmi_sim_manager::mms::MmsContentType;
use qmi_sim_manager::{SmClientHandle, SmError, SmEvent};

use std::env;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_tracing();
    dotenv().ok().unwrap();
    let _dest = env::var("DEST").unwrap();

    let mut client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    let mut events = client_handle.event_stream()?;
    let handle = tokio::spawn(async move {
        let mut count = 0;
        while let Some(event) = events.next().await {
            if let SmEvent::SmsWapPush(wap_push) = &event {
                client.start_network("free").await?;
                let output = client.get_mms(wap_push.content_location.clone()).await?;
                for content in output {
                    dbg!(content.content_type);
                    let path = match content.content_type {
                        Some(MmsContentType::ImageJpeg) => {
                            format!("mmstest_image{:02?}.jpg", count)
                        }
                        Some(MmsContentType::AudioMp4) => format!("mmstest_audio{:02?}.m4a", count),
                        Some(MmsContentType::TextPlain) => format!("mmstest_text{:02?}.txt", count),
                        Some(MmsContentType::ApplicationSmilXml) => {
                            format!("mmstest_smil{:02?}.sml", count)
                        }
                        _ => format!("mmstest_unknown{:02?}.unk", count),
                    };
                    count += 1;
                    tokio::fs::write(path, content.content)
                        .await
                        .map_err(SmError::TokioRuntimeError)?;
                }
                client.stop_network().await?;
            }
            println!("{event:?}");
        }
        Ok::<(), SmError>(())
    });
    tokio::time::sleep(Duration::from_secs(60)).await; // Keep the client running for one minute before shutting it down.
    client_handle.shutdown().await?;
    drop(handle.await?);
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
