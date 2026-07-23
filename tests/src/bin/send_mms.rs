use dotenvy::dotenv;
use qmi_sim_manager::ClientHandle;
use qmi_sim_manager::mms::WspWellKnownContentType::{ImageJpeg, ImagePng, TextPlain};
use qmi_sim_manager::mms::{MmsContent, WspContentType};

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
            vec![
                MmsContent {
                    content: tokio::fs::read("bibendum.jpg").await?,
                    content_type: WspContentType::WspWellKnownContentType(ImageJpeg),
                },
                // MmsContent {
                //     content: tokio::fs::read("1779995393578.jpeg").await?,
                //     content_type: WspContentType::WspWellKnownContentType(ImageJpeg),
                // },
                MmsContent {
                    content: "Ce sont des choses qui arrivent".as_bytes().to_vec(),
                    content_type: WspContentType::WspWellKnownContentType(TextPlain),
                },
                // MmsContent {
                //     content: tokio::fs::read("long-doc.txt").await?,
                //     content_type: WspContentType::WspWellKnownContentType(TextPlain),
                // },
                // MmsContent {
                //     content: "Ce sont des choses qui arrivent".as_bytes().to_vec(),
                //     content_type: WspContentType::WspWellKnownContentType(TextPlain),
                // },
                // MmsContent {
                //     content: tokio::fs::read("bibendum.jpg").await?,
                //     content_type: WspContentType::WspWellKnownContentType(ImageJpeg),
                // },
                MmsContent {
                    content: tokio::fs::read("tux.png").await?,
                    content_type: WspContentType::WspWellKnownContentType(ImagePng),
                },
                // MmsContent {
                //     content: tokio::fs::read("Screenshot_2026-07-09_15-10-08.png").await?,
                //     content_type: WspContentType::WspWellKnownContentType(ImagePng),
                // },
                MmsContent {
                    content: tokio::fs::read("mmstest_audio01.m4a").await?,
                    content_type: WspContentType::Custom(WspContentType::AUDIO_MP4),
                },
                // MmsContent {
                //     content: tokio::fs::read("mmstest_audio01.m4a").await?,
                //     content_type: WspContentType::Custom(WspContentType::AUDIO_MP4),
                // },
                MmsContent {
                    content: tokio::fs::read("mmstest_smil01.mp4").await?,
                    content_type: WspContentType::Custom(WspContentType::VIDEO_MP4),
                },
            ],
            None,
            &[&dest],
        )
        .await;
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
