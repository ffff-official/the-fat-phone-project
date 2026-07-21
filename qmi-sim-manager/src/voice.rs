use crate::{SmEvent, SmResult};

use libqmi_glib_rs::VoiceClient;

use futures::SinkExt;
use futures::channel::mpsc::{self, TryRecvError};

use std::time::Duration;

pub(crate) async fn process_voice_events(
    mut voice_client: VoiceClient,
    mut sm_event_sender: mpsc::UnboundedSender<SmEvent>,
) -> SmResult<()> {
    let (voice_evt_sender, mut voice_evt_receiver) = mpsc::unbounded();
    voice_client.set_all_call_status(voice_evt_sender).await?;
    loop {
        match voice_evt_receiver.try_recv() {
            Ok(call_info_vec) => {
                let _r = sm_event_sender
                    .send(SmEvent::VoiceCallEvent(call_info_vec))
                    .await
                    .inspect_err(|e| tracing::error!("cannot send : {e}"));
            }
            Err(TryRecvError::Empty) => {
                glib::timeout_future(Duration::from_millis(10)).await;
            }
            Err(TryRecvError::Closed) => {
                tracing::debug!("Sender is Droped !");
                break;
            }
        }
    }
    Ok(())
}
