pub(crate) mod mms;
pub(crate) mod sms;

use crate::messaging::mms::WspContentType;
use crate::messaging::sms::SmsType;
use crate::network::WWAN_INTERFACE;
use crate::{Event, SmResult};
pub(crate) use sms::get_sms_deliver_tpdu;

use libqmi_glib_rs::WmsClient;
use libqmi_glib_types_rs::{WmsMessageFormat, WmsReceiptAction};

use mms::m_notification_ind::WapPush;
use mms::m_retrieve_conf::WspBody::WapMultiPart;
use mms::{ResponseStatus, parse_xmms_field};
use sms::IncomingSmsTpdu;

use sms::SmsStatusReport;

use futures::channel::mpsc::{self, TryRecvError, UnboundedReceiver};
use reqwest::Client;
use tokio::runtime::Handle;

use std::fmt;
use std::fmt::Debug;
use std::slice::SliceIndex;
use std::thread::JoinHandle;
use std::time::Duration;

const REQWEST_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) trait SliceResultExt<T, I: SliceIndex<[T]>> {
    fn get_result(&self, idx: I) -> Result<&I::Output, MessageOutOfBound>;
}

impl<T, I: SliceIndex<[T]> + Debug + Clone> SliceResultExt<T, I> for [T] {
    fn get_result(&self, index: I) -> Result<&I::Output, MessageOutOfBound> {
        self.get(index.clone()).ok_or_else(|| MessageOutOfBound {
            index: format!("{:?}", index),
            len: self.len(),
        })
    }
}

/// Returned when decoding attempts to read beyond the available SMS or MMS data.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MessageOutOfBound {
    /// The byte index at which the out-of-bounds access occurred.
    pub index: String,
    /// The total length of the available data, in bytes.
    pub len: usize,
}
impl fmt::Display for MessageOutOfBound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "index {} out of bound : Len = {}", self.index, self.len)
    }
}

impl std::error::Error for MessageOutOfBound {}

/// This function performs the initialization required for libqmi to receive incoming SMS messages and invoke
/// the get_message_data() callback when a new message is received. The routes are currently configured in "transfer-and-ack" mode.
pub(crate) async fn start_messaging(
    wms_client: &mut WmsClient,
    tokio_handle: Handle,
    sm_event_sender: mpsc::Sender<Event>,
) -> SmResult<JoinHandle<()>> {
    tracing::debug!("prepare_messaging :");
    let (mut tpdu_sender, tpdu_receiver) = futures::channel::mpsc::unbounded();
    let mut sm_event_sender_clone = sm_event_sender.clone();
    wms_client
        .set_event_report(
            Box::new(move |result| {
                let ret = match result {
                    Ok((tpdu, format)) => tpdu_sender.start_send((tpdu.to_vec(), format)),
                    Err(e) => sm_event_sender_clone.start_send(Event::InternalError(e.into())),
                };
                if let Err(e) = ret {
                    tracing::warn!("set_event_report sender : cannot send : {:?}", e);
                }
            }),
            (),
        )
        .await?;
    let mut routes = wms_client.get_routes().await?;
    for route in routes.iter_mut() {
        route.receipt_action = WmsReceiptAction::TransferAndAck;
    }
    tracing::debug!("Setting routes");
    wms_client.set_routes(routes).await?;
    tracing::debug!("Setting routes finished");
    let _routes = wms_client.get_routes().await?;

    let tpdu_thread_handler = std::thread::spawn(move || {
        let _b = tokio_handle
            .block_on(async move { process_messsages(tpdu_receiver, sm_event_sender).await });
        tracing::debug!("Tokio block_on exiting wih value = {:?}", _b);
    });
    Ok(tpdu_thread_handler)
}

/// This function receives TPDUs through its MPSC channel for the GLib "event-report" signal handler.
/// When an SMS is received, the receiver is invoked. It parses the TPDU and then sends
/// the result through an MPSC channel to the library client.
async fn process_messsages(
    mut tpdu_reveiver: UnboundedReceiver<(Vec<u8>, WmsMessageFormat)>,
    mut sm_event_sender: mpsc::Sender<Event>,
) -> SmResult<u32> {
    loop {
        match tpdu_reveiver.try_recv() {
            Ok((tpdu, format)) => {
                tracing::debug!("format = {:?}", format);
                drop(tracing_debug_hex(&tpdu));
                match sms::parse_incoming_sms(tpdu.as_slice()) {
                    Ok(tpdu) => {
                        tracing::debug!("\n{}", tpdu);
                        use IncomingSmsTpdu::*;
                        match tpdu {
                            SmsDeliverTpdu(tpdu) => match sms::SmsType::try_from(tpdu) {
                                Ok(sms_type) => {
                                    crate::send_event(
                                        &mut sm_event_sender,
                                        match sms_type {
                                            SmsType::Sms(sms) => Event::Sms(sms),
                                            SmsType::SmsWapPush(sms_wap_push) => {
                                                Event::SmsWapPush(sms_wap_push)
                                            }
                                        },
                                    )
                                    .await
                                }
                                Err(e) => {
                                    crate::send_event(
                                        &mut sm_event_sender,
                                        Event::InternalError(crate::Error::UnexpectedPattern(
                                            e.into(),
                                        )),
                                    )
                                    .await
                                }
                            },
                            SmsStatusReportTpdu(tpdu) => {
                                crate::send_event(
                                    &mut sm_event_sender,
                                    Event::SmsStatusReport(SmsStatusReport::from(tpdu)),
                                )
                                .await
                            }
                        }
                    }
                    Err(e) => {
                        crate::send_event(&mut sm_event_sender, Event::InternalError(e)).await
                    }
                }
            }
            Err(TryRecvError::Empty) => {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            Err(TryRecvError::Closed) => {
                tracing::debug!("Sender is Droped !");
                break;
            }
        }
    }
    Ok(42)
}

/// Retrieves the MMS from the MMSC, parses it, and extracts the contained data.
pub(crate) async fn get_mms(url: String) -> SmResult<Vec<MmsContent>> {
    let wwan = crate::network::WWAN_INTERFACE.get().expect("WTF");

    let client = reqwest::Client::builder()
        .interface(wwan)
        .connect_timeout(REQWEST_TIMEOUT)
        .timeout(REQWEST_TIMEOUT * 2)
        .build()?;
    let response = client.get(url).send().await?;
    if response.status().is_success() {
        tracing::debug!("Writing MMS");

        let content = response.bytes().await?;

        #[cfg(debug_assertions)]
        {
            tracing::warn!("MMS must not be writed here, it is only for debug");
            drop(tokio::fs::write("mms.bin", content.clone()).await);
        }

        let parsed = mms::m_retrieve_conf::WspMRetrieveConf::try_from(content.as_ref())?;
        tracing::debug!("{}", parsed);

        if let WapMultiPart(v) = parsed.body {
            Ok(v.parts
                .into_iter()
                .map(|part| MmsContent {
                    content: part.body.to_vec(),
                    content_type: part.headers.content_type,
                })
                .collect::<Vec<MmsContent>>())
        } else {
            Err("Not a WapMultiPart MMS".into())
        }
    } else {
        let err = response.error_for_status();
        tracing::warn!("Cant fetch ressource at {} err = {:?}", wwan, err);
        Err("Bad response status".into())
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
/// Represents a content part of an MMS.
pub struct MmsContent {
    /// The raw content of the part.
    pub content: Vec<u8>,
    /// The MIME type of the content, if known.
    pub content_type: WspContentType,
}

impl fmt::Debug for MmsContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "content_type = {:?} len = {}",
            self.content_type,
            self.content.len()
        )
    }
}

/// Represents the response returned after submitting an MMS.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct MmsStatus {
    /// The identifier assigned to the submitted MMS.
    pub message_id: String,
    /// The response status returned by the MMSC.
    pub response_status: ResponseStatus,
}

impl MmsContent {
    /// Creates a new MMS content part.
    pub fn new(content: Vec<u8>, content_type: WspContentType) -> Self {
        Self {
            content,
            content_type,
        }
    }
}

/// Send a MMS with specified content TO address.
pub async fn send_mms(
    mmsc: String,
    content: Vec<MmsContent>,
    from: Option<String>,
    to: Vec<String>,
) -> SmResult<MmsStatus> {
    let pdu = mms::m_send_req::new(
        content.as_slice(),
        from.as_deref(),
        &to.iter().map(String::as_str).collect::<Vec<&str>>(),
    )?;

    let client = Client::builder()
        .interface(WWAN_INTERFACE.get().expect("WTF"))
        .connect_timeout(REQWEST_TIMEOUT)
        .timeout(REQWEST_TIMEOUT * 2)
        .build()?;

    let response = client
        .post(mmsc)
        .header("Content-Type", "application/vnd.wap.mms-message")
        .header("Accept", "application/vnd.wap.mms-message")
        .body(pdu)
        .send()
        .await?;

    tracing::debug!("MMS HTTP response status : {}", response.status());
    match response.error_for_status() {
        Ok(response) => {
            let body = response.bytes().await?;

            let mut xmms_field = mms::XMmsFields::default();
            let mut idx = 0;

            while idx < body.len() && parse_xmms_field(&body, &mut xmms_field, &mut idx).is_ok() {}
            tracing::debug!("\n{}", xmms_field);

            // tokio::fs::write("m-send-conf.pdu", &body).await?;
            tracing::debug!("Receive {} bytes", body.len());
            Ok(MmsStatus {
                message_id: xmms_field.message_id.unwrap_or("Unknown".into()),
                response_status: xmms_field
                    .response_status
                    .unwrap_or(ResponseStatus::Unknown),
            })
        }
        Err(e) => Err(e.into()),
    }
}

fn tracing_debug_hex(source: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write;

    let mut hex = String::new();
    writeln!(&mut hex, "source raw values :")?;
    for b in source {
        write!(&mut hex, "{:02X} ", b)?;
    }
    hex.pop();

    tracing::debug!("{hex}");
    Ok(())
}
