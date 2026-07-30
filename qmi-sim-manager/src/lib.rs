//! A Rust library for interacting with QMI-compatible cellular modems.
//!
//! The library provides an asynchronous API for:
//! - Placing, answering, and terminating voice calls.
//! - Sending and receiving SMS messages.
//! - Downloading and sending MMS messages.
//! - Receiving SMS delivery reports.
//! - Receiving MMS notifications (WAP Push).
//! - Managing packet data connections.
//! - Monitoring network registration and connectivity.
//! - Managing UIM (SIM) cards and applications.
//! - Receiving asynchronous modem events, including SMS, MMS, and call status indications.
//!
//! # Beta status
//!
//! The public API is considered stable and is not expected to undergo significant changes before the first stable release.
//!
//! The library is still under active development, and some features are incomplete and can receive improvements based on feedback.
//!
//! # Basics
//!
//! ```no_run
//! use futures::stream::StreamExt;
//! use qmi_sim_manager::ClientHandle;
//!
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Owns the service engine and is responsible for its initialization and shutdown.
//!     let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
//!
//!     // Client is used to call all library methods.
//!     // It is cloneable and can be safely shared across multiple threads.
//!     let client = client_handle.client();
//!
//!     // EventStream is used to receive all events.
//!     // It is not cloneable, but it can be sent to another thread.
//!     let mut events = client_handle.event_stream()?;
//!     let handler = tokio::spawn(async move {
//!         // Process incoming events, such as received SMS messages or ringing.
//!         while let Some(event) = events.next().await {
//!             println!("{event:?}");
//!         }
//!     });
//!     client_handle.shutdown().await?;
//!     handler.await?;
//!     Ok(())
//! }
//! ```
//! # Future development
//!
//! This library is under active development.
//! Future releases are expected to add support for additional modem features, including better SIM card management, mobile network gateway, GPS, and more.
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(clippy::enum_variant_names)]

pub(crate) mod engine;
pub(crate) mod init;
pub(crate) mod messaging;
pub(crate) mod network;

#[cfg(feature = "at-at")]
#[cfg_attr(docsrs, doc(cfg(feature = "at-at")))]
pub mod atat;

const EVT_CHANNEL_LEN: usize = 32;

/// Types related to SMS messages.
pub mod sms {
    pub use crate::messaging::sms::SmsWapPush;
    pub use crate::messaging::sms::{ConcatenatedShortMessages, Sms};
    pub use crate::messaging::sms::{SmsStatusReport, TpStatus};
}
/// Types related to MMS messages.
pub mod mms {
    pub use crate::messaging::mms::{ResponseStatus, WspContentType, WspWellKnownContentType};
    pub use crate::messaging::{MmsContent, MmsStatus};
}
/// Types related to VOICE calls.
pub mod voice {
    pub use libqmi_glib_rs::voice_client::VoiceCall;
}
/// Types related to UIM module.
pub mod uim {
    pub use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElement;
    pub use libqmi_glib_types_rs::MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2;
}
/// Types related to DMS module.
pub mod dms {
    pub use libqmi_glib_rs::dms_client::*;
}
/// Types related to NAS module.
pub mod nas {
    pub use libqmi_glib_rs::nas_client::*;
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

use engine::*;

use futures::channel::mpsc;
use futures::channel::oneshot;
use futures::task::{Context, Poll};
use futures::{SinkExt, Stream};
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
/// Use [`ClientHandle::client`] to obtain a cloneable [`Client`]
/// that can be shared across tasks.
pub struct ClientHandle {
    client: Client,
    events: Option<mpsc::Receiver<Event>>,
    thread: Option<std::thread::JoinHandle<SmResult<()>>>,
}

/// A cloneable client used to interact with the modem.
///
/// This type is cheap to clone and can safely be shared between threads.
#[derive(Clone)]
pub struct Client {
    inner: Arc<SmClientInner>,
}

/// EventStream provides methods for retrieving events asynchronously or non-blockingly through next_event and try_next_event.
/// It also implements the futures::Stream trait.
///
/// **Note**: The channel is bounded and can hold at most 32 pending events.
/// Additional events are dropped until space becomes available.
pub struct EventStream(mpsc::Receiver<Event>);

struct SmClientInner {
    engine_sender: mpsc::UnboundedSender<engine::Request>,
}

/// Events emitted by the modem.
#[derive(Debug)]
pub enum Event {
    /// An SMS has been received.
    Sms(sms::Sms),
    /// A WAP Push SMS has been received.
    SmsWapPush(sms::SmsWapPush),
    /// An SMS status report has been received.
    SmsStatusReport(sms::SmsStatusReport),
    /// Voice call event.
    VoiceCallEvent(Vec<voice::VoiceCall>),
    /// Nas call event.
    NasEvent(nas::NasEventReport),
    /// Nas call event.
    NasSignalInfo(nas::SignalInfo),
    /// Internal Error.
    InternalError(Error),
}

async fn send_event(sender: &mut mpsc::Sender<Event>, evt: Event) {
    let _r = sender
        .send(evt)
        .await
        .inspect_err(|e| tracing::error!("cannot send : {e}"));
}

impl ClientHandle {
    /// Creates a new client connected to the specified QMI device.
    pub fn init(qmi_device_path: impl AsRef<Path>) -> SmResult<Self> {
        let (status_sender, status_receiver) =
            sync_channel::<SmResult<mpsc::UnboundedSender<engine::Request>>>(1);

        let (sm_event_sender, sm_event_receiver) = mpsc::channel(EVT_CHANNEL_LEN);

        let path = qmi_device_path.as_ref().to_path_buf();
        let thread =
            std::thread::spawn(move || init::init_thread(path, status_sender, sm_event_sender));

        let engine_sender = status_receiver
            .recv()
            .map_err(|e| Error::Other(e.to_string()))??;

        let client = Client {
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
    /// The returned [`Client`] can safely be shared between threads.
    pub fn client(&self) -> Client {
        self.client.clone()
    }

    /// Returns the [`EventStream`] used to receive incoming events.
    ///
    /// EventStream is not cloneable, so this method can only be called
    /// successfully once.
    pub fn event_stream(&mut self) -> SmResult<EventStream> {
        match self.events.take() {
            Some(events) => Ok(EventStream(events)),
            None => Err(Error::EventsIteratorAlreadyTaken),
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
            thread.join().map_err(|_| Error::ThreadPanicked)??;
        }

        Ok(())
    }
}

/// Errors returned by this library.
#[derive(Debug, thiserror::Error)]
pub enum Error {
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
    /// Downcasting error on libqmi-rs Service feature
    #[error("Raw service response could not be downcast to the requested type")]
    DowncastingFailed,
    #[cfg(feature = "at-at")]
    #[cfg_attr(docsrs, doc(cfg(feature = "at-at")))]
    /// The Tokio serial crate encountered an error.
    #[error("Tokio Serial Error")]
    TokioSerialError(#[source] tokio_serial::Error),
    #[cfg(feature = "at-at")]
    #[cfg_attr(docsrs, doc(cfg(feature = "at-at")))]
    /// Error from AT-AT.
    #[error("Errpr from AT-AT")]
    AtAtError(atat::Error),
    /// Generic error. This error will be removed in a future version.
    #[error("Generic Error")]
    Other(String),
}

impl From<String> for Error {
    fn from(value: String) -> Self {
        Self::UnexpectedPattern(value)
    }
}

impl From<&'static str> for Error {
    fn from(value: &'static str) -> Self {
        Self::UnexpectedPattern(value.into())
    }
}

impl<T> From<futures::channel::mpsc::TrySendError<T>> for Error {
    fn from(_: futures::channel::mpsc::TrySendError<T>) -> Self {
        Error::EngineStopped
    }
}

impl From<futures::channel::oneshot::Canceled> for Error {
    fn from(_value: futures::channel::oneshot::Canceled) -> Self {
        Error::EngineStopped
    }
}

impl<T> From<num_enum::TryFromPrimitiveError<T>> for Error
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
pub type SmResult<T> = Result<T, Error>;

impl Stream for EventStream {
    type Item = Event;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_next(cx)
    }
}

impl EventStream {
    /// Waits for the next modem event.
    ///
    /// Returns `None` if the event stream has been closed.
    pub async fn next_event(&mut self) -> Option<Event> {
        use futures::StreamExt;
        self.next().await
    }

    /// Attempts to retrieve the next pending event without blocking.
    ///
    /// Returns `Ok(None)` if no event is currently available.
    pub fn try_next_event(&mut self) -> SmResult<Option<Event>> {
        match self.0.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Closed) => Err(Error::EngineStopped),
        }
    }
}

macro_rules! with {
    (
        $(#[$meta:meta])*
        $method_name:ident,
        $client_type:ty,
        $req_variant:path,
        $req_inner_type:ident
    ) => {
        $(#[$meta])*
        pub async fn $method_name<T, F, Fut>(&self, executor: F) -> SmResult<T>
        where
            T: Send + 'static,
            F: FnOnce($client_type, tokio::runtime::Handle) -> Fut + Send + 'static,
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
                .unbounded_send($req_variant($req_inner_type {
                    executor,
                    reply_sender,
                }))?;

            let response = reply_receiver.await?;
            downcast_type(response)
        }
    }
}

impl Client {
    /// Starts a mobile data connection using the specified APN.
    ///
    /// # Parameters
    ///
    /// - `apn`: The Access Point Name (APN) to use.
    ///
    /// # Returns
    ///
    /// Returns the packet data connection status reported by the modem.
    /// ```no_run
    /// # use qmi_sim_manager::ClientHandle;
    /// #
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// # let client = client_handle.client();
    /// #
    /// client.start_network("free").await?;
    /// #
    /// # client_handle.shutdown().await?;
    /// #    Ok(())
    /// # }
    /// ```
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

    /// Stops the current mobile data connection.
    ///
    /// # Returns
    ///
    /// Returns the final packet data connection status reported by the modem.
    /// ```no_run
    /// # use qmi_sim_manager::ClientHandle;
    /// #
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// # let client = client_handle.client();
    /// #
    /// # client.start_network("free").await?;
    /// #
    /// client.stop_network().await?;
    /// #
    /// # client_handle.shutdown().await?;
    /// #    Ok(())
    /// # }
    /// ```
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
    /// Long messages are automatically split into multiple SMS segments if needed.
    ///
    /// # Parameters
    ///
    /// - `dest`: The recipient phone number.
    /// - `content`: The message text.
    /// - `recept_ack`: Requests a delivery report from the network.
    ///
    /// # Returns
    ///
    /// Returns one result per SMS segment. Each successful entry contains the
    /// message reference assigned by the modem.
    /// ```no_run
    /// # use qmi_sim_manager::ClientHandle;
    /// #
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// # let client = client_handle.client();
    /// #
    /// client.send_sms("32712786512", "That's bullshit.", true).await?;
    /// #
    /// # client_handle.shutdown().await?;
    /// #    Ok(())
    /// # }
    /// ```
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

    /// Downloads an MMS from the specified content location.
    ///
    /// The `content_location` is typically obtained from a previously received [`sms::SmsWapPush`] event.
    ///
    /// # Parameters
    ///
    /// - `content_location`: The URL of the MMS to download.
    ///
    /// # Returns
    ///
    /// Returns the decoded MMS content parts.\
    /// ```no_run
    /// # use futures::stream::StreamExt;
    /// # use qmi_sim_manager::ClientHandle;
    /// use qmi_sim_manager::{Error, Event};
    ///
    /// # use std::path::PathBuf;
    /// # use std::time::Duration;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #    let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// #    let client = client_handle.client();
    /// #
    /// // Create the event stream. Only one event stream can exist at a time.
    /// let mut events = client_handle.event_stream()?;
    ///
    /// // Spawn a task that continuously processes modem events.
    /// let handle = tokio::spawn(async move {
    ///     while let Some(event) = events.next().await {
    ///         // A WAP Push notification announces that an MMS is available.
    ///         if let Event::SmsWapPush(wap_push) = &event {
    ///             // A packet data connection is required to download the MMS.
    ///             client.start_network("free").await?;
    ///
    ///             let output = client
    ///                 .get_mms(wap_push.content_location.clone())
    ///                 .await?;
    ///
    ///              // Display the MIME type of each MMS part.
    ///             for content in output {
    ///                 dbg!(&content.content_type);
    ///             }
    ///             client.stop_network().await?;
    ///         }
    ///         println!("{event:?}");
    ///     }
    ///     Ok::<(), Error>(())
    /// });
    /// #    tokio::time::sleep(Duration::from_secs(60)).await;
    /// #    client_handle.shutdown().await?;
    /// #    handle.await??;
    /// #    Ok(())
    /// # }
    /// ```
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

    /// Sends an MMS message.
    ///
    /// The message is sent through the MMSC specified by `mmsc`.
    ///
    /// # Parameters
    ///
    /// - `mmsc`: The URL of the Multimedia Messaging Service Center.
    /// - `content`: The MMS content parts to send.
    /// - `from`: An optional sender address.
    /// - `to`: The list of recipient addresses.
    ///
    /// # Returns
    ///
    /// Returns the MMS status reported by the MMSC when the message has been
    /// successfully processed.
    /// ```no_run
    /// # use qmi_sim_manager::ClientHandle;
    /// use qmi_sim_manager::mms::{MmsContent, WspContentType};
    /// use qmi_sim_manager::mms::WspWellKnownContentType::ImageJpeg;
    ///
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// # let client = client_handle.client();
    /// #
    /// // A mobile data connection is required to send an MMS through the MMSC.
    /// client.start_network("free").await?;
    /// let response = client
    ///     .send_mms(
    ///         "http://mms.free.fr",
    ///         vec![MmsContent {
    ///             content: tokio::fs::read("bibendum.jpg").await?,
    ///             content_type: WspContentType::WspWellKnownContentType(ImageJpeg)
    ///         }],
    ///         None,
    ///         &["33612892311"],
    ///     )
    ///     .await?;
    /// println!("{response:?}");
    /// client.stop_network().await?;
    /// #
    /// # client_handle.shutdown().await?;
    /// #    Ok(())
    /// # }
    /// ```
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

    with!(
    /// Executes a request on the WDS service using [`libqmi::WdsClient`] from *libqmi-glib-rs*.
    ///
    /// The function body executes on the GLib runtime.
    /// The second parameter is a handle to the Tokio runtime, allowing Tokio tasks to be spawned if needed.
    ///
    /// # Warning
    ///
    /// This is a low-level API. The supplied closure is executed by the service
    /// engine and may return any `Send + 'static` value.
    /// ```no_run
    /// # use qmi_sim_manager::ClientHandle;
    /// use qmi_sim_manager::libqmi::types::WdsConnectionStatus;
    ///
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// # let client = client_handle.client();
    /// #
    /// client.start_network("free").await?; // Activate a mobile data connection.
    ///
    /// let result = client
    ///     .with_wds(async move |client, _tk| { client.get_packet_service_status().await })
    ///     .await?;
    /// assert_eq!(result?, WdsConnectionStatus::Connected);
    ///
    /// client.stop_network().await?;
    ///
    /// let result = client
    ///     .with_wds(async move |client, _tk| { client.get_packet_service_status().await })
    ///     .await?;
    /// assert_eq!(result?, WdsConnectionStatus::Disconnected);
    /// #
    /// # client_handle.shutdown().await?;
    /// #   Ok(())
    /// # }
    /// ```
    with_wds, libqmi_glib_rs::WdsClient, Request::WdsService, WdsServiceRequest);

    with!(
    /// Executes a request on the WMS service using [`libqmi::WmsClient`] from *libqmi-glib-rs*.
    ///
    /// The function body executes on the GLib runtime.
    /// The second parameter is a handle to the Tokio runtime, allowing Tokio tasks to be spawned if needed.
    ///
    /// # Warning
    ///
    /// This is a low-level API. The supplied closure is executed by the service
    /// engine and may return any `Send + 'static` value.
    /// ```no_run
    /// # use qmi_sim_manager::ClientHandle;
    /// #
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// # let client = client_handle.client();
    /// #
    /// let routes = client
    ///     .with_wms(async move |wms, _tk| wms.get_routes().await)
    ///     .await??;
    ///
    /// for route in routes {
    ///     println!("{route:#?}");
    /// }
    /// #
    /// # client_handle.shutdown().await?;
    /// # Ok(())
    /// # }
    /// ```
    with_wms, libqmi_glib_rs::WmsClient, Request::WmsService, WmsServiceRequest);

    with!(
    /// Executes a request on the UIM service using [`libqmi::UimClient`] from *libqmi-glib-rs*.
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
    /// # use qmi_sim_manager::ClientHandle;
    /// # use std::path::PathBuf;
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// # let client = client_handle.client();
    /// #
    /// // Retrieve the status of the available UIM cards and their applications.
    /// let card_statuses = client
    ///     .with_uim(async move |client, _tk| {
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
    ///     .with_uim(async move |client, _tk| {
    ///         client.verify_pin("1234").await
    ///     })
    ///     .await??;
    ///
    /// println!("Remaining PIN retries: {remaining_retries}");
    /// #
    /// # client_handle.shutdown().await?;
    /// # Ok(())
    /// # }
    /// ```
    with_uim, libqmi_glib_rs::UimClient, Request::UimService, UimServiceRequest);

    with!(
    /// Executes a request on the QMI Voice service using [`libqmi::VoiceClient`] from *libqmi-glib-rs*.
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
    /// # use futures::stream::StreamExt;
    /// # use qmi_sim_manager::{Client, ClientHandle};
    /// use qmi_sim_manager::Event::VoiceCallEvent;
    /// use qmi_sim_manager::libqmi::types::VoiceCallState;
    ///
    /// # use std::path::PathBuf;
    /// use std::time::Duration;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;
    /// #
    /// # let client = client_handle.client();
    /// #
    /// let mut events = client_handle.event_stream()?;
    /// let handler = tokio::spawn(async move {
    ///     // Process voice call events reported by the modem.
    ///     while let Some(event) = events.next().await {
    ///         if let VoiceCallEvent(items) = event {
    ///             for item in items {
    ///                 let client_clone = client.clone();
    ///                 println!("{:#?}", item);
    ///                 if item.state == VoiceCallState::Incoming {
    ///                     dbg!("incoming call received");
    ///                     if let Err(e) = tokio::spawn(answer(client_clone, item.id)).await {
    ///                         eprintln!("Exiting with error : {e:?}");
    ///                     }
    ///                 }
    ///             }
    ///         }
    ///     }
    /// });
    /// async fn answer(client: Client, id: u8) -> Result<(), qmi_sim_manager::Error> {
    ///     client
    ///         .with_voice(async move |client, _tk| client.answer_call(id).await)
    ///         .await??;
    ///     // Keep the call active for ten seconds.
    ///     tokio::time::sleep(Duration::from_secs(10)).await;
    ///     client
    ///         .with_voice(async move |client, _tk| client.end_call(id).await)
    ///         .await??;
    ///     Ok(())
    /// }
    /// // Keep the client running for one minute before shutting it down.
    /// tokio::time::sleep(Duration::from_secs(60)).await;
    /// client_handle.shutdown().await?;
    /// handler.await?;
    /// #    Ok(())
    /// # }
    /// ```        
    with_voice, libqmi_glib_rs::VoiceClient, Request::VoiceService, VoiceServiceRequest);

    with!(
    /// TODO : Documentation
    with_dms, libqmi_glib_rs::DmsClient, Request::DmsService, DmsServiceRequest);

    with!(
    /// TODO : Documentation
    with_nas, libqmi_glib_rs::NasClient, Request::NasService, NasServiceRequest);
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
        .map_err(|_| Error::DowncastingFailed)
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
