//! A Rust library for interacting with QMI-compatible cellular modems.
//!
//! The library provides an asynchronous API for:
//! - Sending and receiving SMS messages.
//! - Receiving SMS delivery reports.
//! - Receiving MMS notifications (WAP Push).
//! - Downloading and sending MMS messages.
//! - Managing packet data connections.
//! - Monitoring network registration and connectivity.
//! - Managing UIM (SIM) cards and applications.
//! - Placing, answering, and terminating voice calls.
//! - Receiving asynchronous modem events, including SMS, MMS, and call status indications.
//!
//! # Alpha status
//!
//! This library is currently in an alpha stage. Its API and behavior may change before the first stable release.
//!
//! At the moment, MMS sending is limited to image attachments and SIM features are limited.
//!
//! # Example
//!
//! ```no_run
//! use futures::stream::StreamExt;
//! use qmi_sim_manager::SmClientHandle;
//! use qmi_sim_manager::mms::{MmsContent, MmsContentType};
//!
//! use std::path::PathBuf;
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
//!
//!     let client = client_handle.client();
//!
//!     client.send_sms("33612402244", "banane", true).await?;
//!
//!     client.start_network("free").await?; // A mobile data connection is required to send an MMS through the MMSC.
//!     let response = client
//!         .send_mms(
//!             "http://mms.free.fr",
//!             vec![MmsContent {
//!                 content: tokio::fs::read("bibendum.jpg").await?,
//!                 content_type: Some(MmsContentType::ImageJpeg),
//!             }],
//!             None,
//!             &["33612402244"],
//!         )
//!         .await?;
//!     println!("{:?}", response);
//!     client.stop_network().await?;
//!
//!     let mut events = client_handle.event_stream()?;
//!     let handler = tokio::spawn(async move {
//!         // Process incoming events, such as received SMS messages or ringing.
//!         while let Some(event) = events.next().await {
//!             println!("{event:?}");
//!         }
//!     });
//!     tokio::time::sleep(Duration::from_secs(60)).await; // Keep the client running for one minute before shutting it down.
//!     client_handle.shutdown().await?;
//!     handler.await?;
//!     Ok(())
//! }
//! ```
//! # Future development
//!
//! This library is under active development.
//! Future releases are expected to add support for additional modem features, including better SIM card management, mobile network gateway, GPS, and more.
#![allow(clippy::enum_variant_names)]

pub(crate) mod engine;
pub(crate) mod init;
pub(crate) mod messaging;
pub(crate) mod network;

/// Types related to SMS messages.
pub mod sms {
    pub use crate::messaging::sms::SmsWapPush;
    pub use crate::messaging::sms::{ConcatenatedShortMessages, Sms};
    pub use crate::messaging::sms::{SmsStatusReport, TpStatus};
}
/// Types related to MMS messages.
pub mod mms {
    pub use crate::messaging::mms::ResponseStatus;
    pub use crate::messaging::{MmsContent, MmsContentType, MmsStatus};
}
/// Types related to VOICE calls.
pub mod voice {
    pub use libqmi_glib_rs::voice_client::VoiceCall;
}
/// Types related to UIM.
pub mod uim {
    pub use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElement;
    pub use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2;
}
/// *Native public import from libqmi-glib-rs.*
pub mod libqmi {
    pub use libqmi_glib_rs::{Device, NasClient, UimClient, VoiceClient, WdsClient, WmsClient};
    pub use libqmi_glib_rs::{DmsClient, PdsClient};
    pub mod types {
        pub use libqmi_glib_types_rs::*;
    }
}
/// *Error types exposed by the library.*
pub mod error {
    /// This error is returned when an internal conversion fails while decoding an SMS or MMS field.
    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub struct Primitive {
        pub enum_type: &'static str,
        pub invalid_value: String,
    }
    pub use crate::messaging::MessageOutOfBound;
    /// Errors from libqmu-glib-rs.
    pub use libqmi_glib_rs::QmiError;
}
/// Primitive Result Type from libqmi-glib-rs
pub use libqmi_glib_rs::QmiResult;

use engine::{GetMmsRequest, Request, SendMmsRequest, ShutDownRequest, SmsRequest};
use engine::{StartNetworkRequest, StopNetworkRequest};
use engine::{UimServiceRequest, VoiceServiceRequest, WdsServiceRequest, WmsServiceRequest};

use futures::Stream;
use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::task::{Context, Poll};
use num_enum::TryFromPrimitiveError;

use std::any::type_name;
use std::ffi::FromBytesUntilNulError;
use std::path::Path;
use std::pin::Pin;
use std::str::Utf8Error;
use std::string::{FromUtf8Error, FromUtf16Error};
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

/// The main handle for interacting with the client.
///
/// This type owns the background thread and the event stream.
/// Use [`SmClientHandle::client`] to obtain a cloneable [`SmClient`]
/// that can be shared across tasks.
pub struct SmClientHandle {
    client: SmClient,
    events: Option<mpsc::UnboundedReceiver<SmEvent>>,
    thread: Option<std::thread::JoinHandle<SmResult<()>>>,
}

/// A cloneable client used to interact with the modem.
///
/// This type is cheap to clone and can safely be shared between threads.
#[derive(Clone)]
pub struct SmClient {
    inner: Arc<SmClientInner>,
}

/// EventStream provides methods for retrieving events asynchronously or non-blockingly through next_event and try_next_event.
/// It also implements the futures::Stream trait.
pub struct EventStream(mpsc::UnboundedReceiver<SmEvent>);

struct SmClientInner {
    engine_sender: mpsc::UnboundedSender<engine::Request>,
}

/// Events emitted by the modem.
#[derive(Debug)]
pub enum SmEvent {
    /// An SMS has been received.
    Sms(sms::Sms),
    /// A WAP Push SMS has been received.
    SmsWapPush(sms::SmsWapPush),
    /// An SMS status report has been received.
    SmsStatusReport(sms::SmsStatusReport),
    /// Voice call event
    VoiceCallEvent(Vec<voice::VoiceCall>),
}

impl SmClientHandle {
    /// Creates a new client connected to the specified QMI device.
    pub fn init(qmi_device_path: impl AsRef<Path>) -> SmResult<Self> {
        let (status_sender, status_receiver) =
            sync_channel::<SmResult<mpsc::UnboundedSender<engine::Request>>>(1);

        let (sm_event_sender, sm_event_receiver) = mpsc::unbounded();

        let path = qmi_device_path.as_ref().to_path_buf();
        let thread =
            std::thread::spawn(move || init::init_thread(path, status_sender, sm_event_sender));

        let engine_sender = status_receiver
            .recv()
            .map_err(|e| SmError::Other(Box::new(e)))??;

        let client = SmClient {
            inner: Arc::new(SmClientInner { engine_sender }),
        };

        Ok(Self {
            client,
            events: Some(sm_event_receiver),
            thread: Some(thread),
        })
    }

    /// Returns a cloneable client handle.
    ///
    /// The returned [`SmClient`] can safely be shared between threads.
    pub fn client(&self) -> SmClient {
        self.client.clone()
    }

    /// Returns the [`EventStream`] used to receive incoming events.
    ///
    /// EventStream is not cloneable, so this method can only be called
    /// successfully once.
    pub fn event_stream(&mut self) -> SmResult<EventStream> {
        match self.events.take() {
            Some(events) => Ok(EventStream(events)),
            None => Err(SmError::EventsIteratorAlreadyTaken),
        }
    }

    /// Gracefully shuts down the client.
    ///
    /// This waits for the background thread to terminate before returning.
    pub async fn shutdown(mut self) -> SmResult<()> {
        let (reply_sender, reply_receiver) = oneshot::channel::<()>();

        self.client
            .inner
            .engine_sender
            .unbounded_send(Request::ShutDown(ShutDownRequest {
                reason: "User request".into(),
                reply_sender,
            }))?;

        reply_receiver.await?;

        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| SmError::ThreadPanicked)??;
        }

        Ok(())
    }
}

/// Errors returned by this library.
#[derive(Debug, thiserror::Error)]
pub enum SmError {
    /// This error originates from libQmi.
    #[error(transparent)]
    Qmi(#[from] error::QmiError),
    /// This error occurs when a PDU or WSP body has an unexpected format.
    #[error(transparent)]
    OutOfBounds(#[from] error::MessageOutOfBound),
    /// A value in the PDU/WSP does not match the expected values.
    #[error("TryFrom error")]
    TryFromPrimitiveError(error::Primitive),
    /// A value in the PDU/WSP does not match the expected values.
    #[error("Unexpected Pattern")]
    UnexpectedPattern(String),
    /// An invalid UTF-8 string is present in the MMS.
    #[error(transparent)]
    Utf8Error(#[from] Utf8Error),
    /// An invalid UTF-8 string is present in the XML structure of an MMS.
    #[error(transparent)]
    FromUtf8Error(#[from] FromUtf8Error),
    /// A UCS-2/UTF16 SMS contains invalid byte sequences.
    #[error("Invalid UCS2/UTF16 SMS content")]
    FromUtf16Error(#[from] FromUtf16Error),
    /// A network communication issue occurred during a GET or POST request.
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    /// An internal error occurred while parsing or encoding a GSM 7-bit SMS.
    #[error("GSM7")]
    Gsm7Error(#[source] std::io::Error),
    /// The Tokio runtime encountered an error.
    #[error("Tokio Runtime Error")]
    TokioRuntimeError(#[source] std::io::Error),
    /// Failed to initialize the Internet connection through the modem.
    #[error("Network configuration Error")]
    NetworkConfError(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// Failed to write or extract the XML structure of the MMS.
    #[error(transparent)]
    MmsXmlError(#[from] xmltree::Error),
    /// The destination field is incorrect.
    #[error("Invalid Destination name")]
    InvalidDestination(String),
    /// An XMms field name does not end with 0x00.
    #[error(transparent)]
    FromBytesUntilNulError(#[from] FromBytesUntilNulError),
    /// Communication with the engine has been lost.
    #[error("EngineStopped")]
    EngineStopped,
    /// The library's main GLib thread encountered a panic.
    #[error("SM thread panicked")]
    ThreadPanicked,
    /// You can't get the events iterator twice.
    #[error("Events iterator already taken")]
    EventsIteratorAlreadyTaken,
    /// Downcasting error on Raw Service feature
    #[error("Raw service response could not be downcast to the requested type")]
    DowncastingFailed,
    /// Generic error. This error will be removed in a future version.
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

impl From<String> for SmError {
    fn from(value: String) -> Self {
        Self::UnexpectedPattern(value)
    }
}

impl From<&'static str> for SmError {
    fn from(value: &'static str) -> Self {
        Self::UnexpectedPattern(value.into())
    }
}

impl From<futures::channel::mpsc::TrySendError<Request>> for SmError {
    fn from(_: futures::channel::mpsc::TrySendError<Request>) -> Self {
        SmError::EngineStopped
    }
}

impl From<futures::channel::oneshot::Canceled> for SmError {
    fn from(_value: futures::channel::oneshot::Canceled) -> Self {
        SmError::EngineStopped
    }
}

impl<T> From<num_enum::TryFromPrimitiveError<T>> for SmError
where
    T: num_enum::TryFromPrimitive,
    T::Primitive: std::fmt::Debug,
{
    fn from(error: TryFromPrimitiveError<T>) -> Self {
        Self::TryFromPrimitiveError(error::Primitive {
            enum_type: type_name::<T>(),
            invalid_value: format!("{:?}", error.number),
        })
    }
}

/// Result type for this library.
pub type SmResult<T> = Result<T, SmError>;

impl Stream for EventStream {
    type Item = SmEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_next(cx)
    }
}

impl EventStream {
    /// Waits for the next modem event.
    ///
    /// Returns `None` if the event stream has been closed.
    pub async fn next_event(&mut self) -> Option<SmEvent> {
        use futures::StreamExt;
        self.next().await
    }

    /// Attempts to retrieve the next pending event without blocking.
    ///
    /// Returns `Ok(None)` if no event is currently available.
    pub fn try_next_event(&mut self) -> SmResult<Option<SmEvent>> {
        match self.0.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Closed) => Err(SmError::EngineStopped),
        }
    }
}

impl SmClient {
    /// Starts the mobile data connection.
    ///
    /// You must specify the APN name.
    pub async fn start_network(
        &self,
        apn: impl AsRef<str>,
    ) -> SmResult<libqmi::types::WdsConnectionStatus> {
        let (reply_sender, reply_receiver) =
            oneshot::channel::<SmResult<libqmi::types::WdsConnectionStatus>>();

        self.inner
            .engine_sender
            .unbounded_send(Request::StartNetwork(StartNetworkRequest {
                apn_name: apn.as_ref().to_owned(),
                reply_sender,
            }))?;

        reply_receiver.await?
    }

    /// Stops the mobile data connection.
    pub async fn stop_network(&self) -> SmResult<libqmi::types::WdsConnectionStatus> {
        let (reply_sender, reply_receiver) =
            oneshot::channel::<SmResult<libqmi::types::WdsConnectionStatus>>();

        self.inner
            .engine_sender
            .unbounded_send(Request::StopNetwork(StopNetworkRequest { reply_sender }))?;

        reply_receiver.await?
    }

    /// Sends an SMS message.
    ///
    /// If `recept_ack` is `true`, a delivery report is requested from the network.
    pub async fn send_sms(
        &self,
        dest: impl AsRef<str>,
        content: impl AsRef<str>,
        recept_ack: bool,
    ) -> SmResult<Vec<SmResult<u64>>> {
        let (reply_sender, reply_receiver) = oneshot::channel::<SmResult<Vec<SmResult<u64>>>>();

        self.inner
            .engine_sender
            .unbounded_send(Request::SendSms(SmsRequest {
                dest: dest.as_ref().to_owned(),
                content: content.as_ref().to_owned(),
                recept_ack,
                reply_sender,
            }))?;

        reply_receiver.await?
    }

    /// Downloads an MMS from its content location.
    pub async fn get_mms(
        &self,
        content_location: impl AsRef<str>,
    ) -> SmResult<Vec<mms::MmsContent>> {
        let (reply_sender, reply_receiver) = oneshot::channel::<SmResult<Vec<mms::MmsContent>>>();

        self.inner
            .engine_sender
            .unbounded_send(Request::GetMms(GetMmsRequest {
                content_location: content_location.as_ref().to_owned(),
                reply_sender,
            }))?;

        reply_receiver.await?
    }

    /// Sends an MMS.
    ///
    /// `mmsc` is the URL of the mobile provider MMSC used to submit the message.
    /// `to` contains the recipient phone numbers.
    pub async fn send_mms(
        &self,
        mmsc: impl AsRef<str>,
        content: Vec<mms::MmsContent>,
        from: Option<String>,
        to: &[impl AsRef<str>],
    ) -> SmResult<mms::MmsStatus> {
        let (reply_sender, reply_receiver) = oneshot::channel::<SmResult<mms::MmsStatus>>();

        self.inner
            .engine_sender
            .unbounded_send(Request::SendMms(SendMmsRequest {
                mmsc: mmsc.as_ref().to_owned(),
                content,
                from,
                to: to
                    .iter()
                    .map(|v| v.as_ref().to_owned())
                    .collect::<Vec<String>>(),
                reply_sender,
            }))?;

        reply_receiver.await?
    }

    /// Make a raw request to the WDS service through libqmi-glib-rs.
    ///
    /// The function body executes on the GLib runtime.
    /// The second parameter is a handle to the Tokio runtime, allowing Tokio tasks to be spawned if needed.
    ///
    /// # Warning
    ///
    /// This is a low-level API. The supplied closure is executed by the service
    /// engine and may return any `Send + 'static` value.
    /// ```no_run
    /// use qmi_sim_manager::{SmClientHandle, libqmi::types::WdsConnectionStatus};
    /// use std::path::PathBuf;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// let client = client_handle.client();
    ///
    /// client.start_network("free").await?; // Activate a mobile data connection.
    ///
    /// let result = client
    ///     .with_wds(|client, _tk| async move { client.get_packet_service_status().await })
    ///     .await?;
    /// assert_eq!(result?, WdsConnectionStatus::Connected);
    ///
    /// client.stop_network().await?;
    ///
    /// let result = client
    ///     .with_wds(|client, _tk| async move { client.get_packet_service_status().await })
    ///     .await?;
    /// assert_eq!(result?, WdsConnectionStatus::Disconnected);
    ///
    /// client_handle.shutdown().await?;
    /// #   Ok(())
    /// # }
    /// ```
    pub async fn with_wds<T, F, Fut>(&self, executor: F) -> SmResult<T>
    where
        T: Send + 'static,
        F: FnOnce(libqmi_glib_rs::WdsClient, tokio::runtime::Handle) -> Fut + Send + 'static,
        Fut: Future<Output = T> + 'static,
    {
        let executor = Box::new(move |client, tokio_handle| {
            let future = executor(client, tokio_handle);

            Box::pin(async move {
                let response = future.await;
                Box::new(response) as ServiceResult
            }) as ServiceFuture
        });

        let (reply_sender, reply_receiver) = oneshot::channel::<ServiceResult>();
        self.inner
            .engine_sender
            .unbounded_send(Request::WdsService(WdsServiceRequest {
                executor,
                reply_sender,
            }))?;

        let response = reply_receiver.await?;
        downcast_type(response)
    }

    /// Make a raw request to the WMS service through libqmi-glib-rs.
    ///
    /// The function body executes on the GLib runtime.
    /// The second parameter is a handle to the Tokio runtime, allowing Tokio tasks to be spawned if needed.
    ///
    /// # Warning
    ///
    /// This is a low-level API. The supplied closure is executed by the service
    /// engine and may return any `Send + 'static` value.
    pub async fn with_wms<T, F, Fut>(&self, executor: F) -> SmResult<T>
    where
        T: Send + 'static,
        F: FnOnce(libqmi_glib_rs::WmsClient, tokio::runtime::Handle) -> Fut + Send + 'static,
        Fut: Future<Output = T> + 'static,
    {
        let executor = Box::new(move |client, tokio_handle| {
            let future = executor(client, tokio_handle);

            Box::pin(async move {
                let response = future.await;
                Box::new(response) as ServiceResult
            }) as ServiceFuture
        });

        let (reply_sender, reply_receiver) = oneshot::channel::<ServiceResult>();
        self.inner
            .engine_sender
            .unbounded_send(Request::WmsService(WmsServiceRequest {
                executor,
                reply_sender,
            }))?;

        let response = reply_receiver.await?;
        downcast_type(response)
    }

    /// Makes a raw request to the UIM service through `libqmi-glib-rs`.
    ///
    /// The function body executes on the GLib runtime.
    /// The second parameter is a handle to the Tokio runtime, allowing Tokio tasks
    /// to be spawned if needed.
    ///
    /// # Warning
    ///
    /// This is a low-level API. The supplied closure is executed by the service
    /// engine and may return any `Send + 'static` value.
    ///
    /// ```no_run
    /// use qmi_sim_manager::SmClientHandle;
    /// use std::path::PathBuf;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// let client = client_handle.client();
    ///
    /// // Retrieve the status of the available UIM cards and their applications.
    /// let card_statuses = client
    ///     .with_uim(|client, _tk| async move {
    ///         client.get_card_status().await
    ///     })
    ///     .await??;
    ///
    /// for card in card_statuses {
    ///     println!("{card:#?}");
    /// }
    ///
    /// // Verify the PIN1 code of the selected UIM application.
    /// let remaining_retries = client
    ///     .with_uim(|client, _tk| async move {
    ///         client.verify_pin("1234").await
    ///     })
    ///     .await??;
    ///
    /// println!("Remaining PIN retries: {remaining_retries}");
    ///
    /// client_handle.shutdown().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_uim<T, F, Fut>(&self, executor: F) -> SmResult<T>
    where
        T: Send + 'static,
        F: FnOnce(libqmi_glib_rs::UimClient, tokio::runtime::Handle) -> Fut + Send + 'static,
        Fut: Future<Output = T> + 'static,
    {
        let executor = Box::new(move |client, tokio_handle| {
            let future = executor(client, tokio_handle);

            Box::pin(async move {
                let response = future.await;
                Box::new(response) as ServiceResult
            }) as ServiceFuture
        });

        let (reply_sender, reply_receiver) = oneshot::channel::<ServiceResult>();
        self.inner
            .engine_sender
            .unbounded_send(Request::UimService(UimServiceRequest {
                executor,
                reply_sender,
            }))?;

        let response = reply_receiver.await?;
        downcast_type(response)
    }

    /// Makes a raw request to the QMI Voice service through `libqmi-glib-rs`.
    ///
    /// The function body executes on the GLib runtime.
    /// The second parameter is a handle to the Tokio runtime, allowing Tokio tasks
    /// to be spawned if needed.
    ///
    /// # Warning
    ///
    /// This is a low-level API. The supplied closure is executed by the service
    /// engine and may return any `Send + 'static` value.
    ///
    /// ```no_run
    /// use qmi_sim_manager::{SmClientHandle, SmEvent};
    /// use std::{path::PathBuf, time::Duration};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// let client = client_handle.client();
    /// let mut events = client.event_stream();
    ///
    /// // Enable asynchronous call-status indications.
    /// client
    ///     .with_voice(|client, _tk| async move {
    ///         client.set_all_call_status(true).await
    ///     })
    ///     .await??;
    ///
    /// // Start an outgoing call.
    /// let call_id = client
    ///     .with_voice(|client, _tk| async move {
    ///         client.dial_call("0612345678").await
    ///     })
    ///     .await??;
    ///
    /// // Wait until the modem reports that the call has started.
    /// while let Some(event) = events.next_event().await {
    ///     if let SmEvent::VoiceCallEvent(call_event) = event {
    ///         if call_event.is_call_begin(call_id) {
    ///             tokio::time::sleep(Duration::from_secs(10)).await;
    ///             break;
    ///         }
    ///     }
    /// }
    ///
    /// // Terminate the call after ten seconds.
    /// client
    ///     .with_voice(move |client, _tk| async move {
    ///         client.end_call(call_id).await
    ///     })
    ///     .await??;
    ///
    /// client_handle.shutdown().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn with_voice<T, F, Fut>(&self, executor: F) -> SmResult<T>
    where
        T: Send + 'static,
        F: FnOnce(libqmi_glib_rs::VoiceClient, tokio::runtime::Handle) -> Fut + Send + 'static,
        Fut: Future<Output = T> + 'static,
    {
        let executor = Box::new(move |client, tokio_handle| {
            let future = executor(client, tokio_handle);

            Box::pin(async move {
                let response = future.await;
                Box::new(response) as ServiceResult
            }) as ServiceFuture
        });

        let (reply_sender, reply_receiver) = oneshot::channel::<ServiceResult>();
        self.inner
            .engine_sender
            .unbounded_send(Request::VoiceService(VoiceServiceRequest {
                executor,
                reply_sender,
            }))?;

        let response = reply_receiver.await?;
        downcast_type(response)
    }
}

type ServiceResult = Box<dyn std::any::Any + Send + 'static>;
type ServiceFuture = Pin<Box<dyn Future<Output = ServiceResult> + 'static>>;

fn downcast_type<T>(response: Box<dyn std::any::Any + Send>) -> SmResult<T>
where
    T: Send + 'static,
{
    response
        .downcast::<T>()
        .map(|value| *value)
        .map_err(|_| SmError::DowncastingFailed)
}

#[cfg(test)]
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
