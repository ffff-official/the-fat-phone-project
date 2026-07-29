use crate::{Error, ServiceFuture, ServiceResult, SmResult, messaging, network};
use messaging::{MmsContent, MmsStatus};

use libqmi_glib_rs::{NasClient, UimClient, VoiceClient, WdsClient, WmsClient};
use libqmi_glib_types_rs::WdsConnectionStatus;

use futures::channel::mpsc;
use futures::channel::mpsc::TryRecvError;
use futures::channel::oneshot;
use futures::channel::oneshot::Sender;
use nix::libc;
use nix::libc::SIGINT;

use std::fmt;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;

thread_local! {
    pub static GLIB_CONTROL_LOOP_TIMER_COND: AtomicBool = const { AtomicBool::new(true) };
}
const GLIB_CONTROL_LOOP_PERIOD_MS: Duration = Duration::from_millis(10);

/// Request–Reply System
#[derive(Debug)]
pub enum Request {
    ShutDown(ShutDownRequest),
    StartNetwork(StartNetworkRequest),
    StopNetwork(StopNetworkRequest),
    SendSms(SmsRequest),
    GetMms(GetMmsRequest),
    SendMms(SendMmsRequest),
    WdsService(WdsServiceRequest),
    WmsService(WmsServiceRequest),
    UimService(UimServiceRequest),
    VoiceService(VoiceServiceRequest),
    NasService(NasServiceRequest),
}

#[derive(Debug)]
pub struct StopNetworkRequest {
    pub reply_sender: oneshot::Sender<SmResult<WdsConnectionStatus>>,
}

#[derive(Debug)]
pub struct StartNetworkRequest {
    pub apn_name: String,
    pub reply_sender: oneshot::Sender<SmResult<WdsConnectionStatus>>,
}

#[derive(Debug)]
pub struct ShutDownRequest {
    pub reason: String,
    pub reply_sender: oneshot::Sender<()>,
}

#[derive(Debug)]
pub struct SmsRequest {
    pub dest: String,
    pub content: String,
    pub recept_ack: bool,
    pub reply_sender: oneshot::Sender<SmResult<Vec<SmResult<u64>>>>,
}

#[derive(Debug)]
pub struct GetMmsRequest {
    pub content_location: String,
    pub reply_sender: oneshot::Sender<SmResult<Vec<MmsContent>>>,
}

#[derive(Debug)]
pub struct SendMmsRequest {
    pub mmsc: String,
    pub content: Vec<MmsContent>,
    pub from: Option<String>,
    pub to: Vec<String>,
    pub reply_sender: oneshot::Sender<SmResult<MmsStatus>>,
}

macro_rules! with_service_request {
    (
        $request_type:ident,
        $client_type:ty,
        $context:literal
    ) => {
        pub struct $request_type {
            pub executor:
                Box<dyn FnOnce($client_type, tokio::runtime::Handle) -> ServiceFuture + Send>,
            pub reply_sender: oneshot::Sender<ServiceResult>,
        }

        impl fmt::Debug for $request_type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!($context, " service executor"))
            }
        }
    };
}

with_service_request!(WdsServiceRequest, WdsClient, "wds");
with_service_request!(WmsServiceRequest, WmsClient, "wms");
with_service_request!(UimServiceRequest, UimClient, "uim");
with_service_request!(VoiceServiceRequest, VoiceClient, "voice");
with_service_request!(NasServiceRequest, NasClient, "nas");

/// This control loop receives all external commands. It uses the various QMI services to process
/// each command and then returns the result using a Request–Reply pattern.
pub(crate) async fn glib_control_loop(
    mut wds_client: WdsClient,
    mut wms_client: WmsClient,
    uim_client: UimClient,
    voice_client: VoiceClient,
    nas_client: NasClient,
    tokio_handle: tokio::runtime::Handle,
    mut mpsc_receiver: mpsc::UnboundedReceiver<Request>,
) -> SmResult<String> {
    while GLIB_CONTROL_LOOP_TIMER_COND.with(|v| v.load(Relaxed)) {
        let req = mpsc_receiver.try_recv();
        if let Ok(req) = &req {
            tracing::debug!("glib_control_loop = {:?}", req);
        }
        match req {
            Ok(req) => match req {
                Request::StartNetwork(req) => {
                    let ret = network::start_network(
                        &mut wds_client,
                        tokio_handle.clone(),
                        &req.apn_name,
                    )
                    .await;
                    req_send(req.reply_sender, ret);
                }
                Request::StopNetwork(req) => {
                    let ret = network::stop_network(&mut wds_client).await;
                    req_send(req.reply_sender, ret);
                }
                Request::ShutDown(req) => {
                    req_send(req.reply_sender, ());
                    unsafe { libc::raise(SIGINT) };
                    return Ok(req.reason);
                }
                Request::SendSms(req) => {
                    match messaging::get_sms_deliver_tpdu(&req.dest, &req.content, req.recept_ack) {
                        Ok(pdu_vec) => {
                            let mut output = Vec::with_capacity(pdu_vec.len());
                            for pdu in pdu_vec {
                                output.push(
                                    wms_client.send_sms(pdu).await.flatten().map_err(Error::Qmi),
                                );
                            }
                            req_send(req.reply_sender, Ok(output));
                        }
                        Err(e) => req_send(req.reply_sender, Err(e)),
                    }
                }
                Request::GetMms(req) => {
                    let ret = tokio_handle
                        .spawn(messaging::get_mms(req.content_location))
                        .await
                        .map_err(|arg0: tokio::task::JoinError| {
                            Error::TokioRuntimeError(arg0.into())
                        })?;

                    req_send(req.reply_sender, ret);
                }
                Request::SendMms(req) => {
                    let ret = tokio_handle
                        .spawn(messaging::send_mms(req.mmsc, req.content, req.from, req.to))
                        .await
                        .map_err(|arg0: tokio::task::JoinError| {
                            Error::TokioRuntimeError(arg0.into())
                        })?;

                    req_send(req.reply_sender, ret);
                }
                Request::WdsService(req) => {
                    let func = req.executor;
                    let ret = func(wds_client.clone(), tokio_handle.clone()).await;
                    req_send(req.reply_sender, ret);
                }
                Request::WmsService(req) => {
                    let func = req.executor;
                    let ret = func(wms_client.clone(), tokio_handle.clone()).await;
                    req_send(req.reply_sender, ret);
                }
                Request::UimService(req) => {
                    let func = req.executor;
                    let ret = func(uim_client.clone(), tokio_handle.clone()).await;
                    req_send(req.reply_sender, ret);
                }
                Request::VoiceService(req) => {
                    let func = req.executor;
                    let ret = func(voice_client.clone(), tokio_handle.clone()).await;
                    req_send(req.reply_sender, ret);
                }
                Request::NasService(req) => {
                    let func = req.executor;
                    let ret = func(nas_client.clone(), tokio_handle.clone()).await;
                    req_send(req.reply_sender, ret);
                }
            },
            Err(TryRecvError::Empty) => {
                glib::timeout_future(GLIB_CONTROL_LOOP_PERIOD_MS).await;
            }
            Err(e) => Err(e).map_err(|e| Error::Other(e.to_string()))?,
        }
    }
    Ok("GLIB_CONTROL_LOOP_TIMER_COND set to FALSE".into())
}

fn req_send<T: std::fmt::Debug>(reply_sender: Sender<T>, ret: T) {
    if let Err(e) = reply_sender.send(ret) {
        tracing::error!("cannot send : {e:?}");
    }
}
