//! # This module provides a simple interface for sending basic AT commands to the modem.
//!
//! ![Architecture](https://gitlab.com/mordaklavache/the-fat-phone-project/-/raw/main/assets/at-at.png?ref_type=heads)
//!
//! It is built on top of [`atat`](https://crates.io/crates/atat/0.24.1) crate
//! and related crates designed especially for async communication, exposing only a minimal
//! subset of their functionality required for basic AT command communication.
//!
//! Enable this module with the **`at-at`** Cargo feature.
//! # Basics
//!
//! ```no_run
//! use futures::StreamExt;
//! use std::path::PathBuf;
//!
//! // You need to import the following items:
//! use qmi_sim_manager::atat::{self, AtClientHandle, atat_derive::{AtatCmd, AtatResp}};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Owns the service engine and is responsible for its initialization and shutdown.
//!     let mut at_client_handle = AtClientHandle::init(PathBuf::from("/dev/ttyUSB3"), 115200)?;
//!
//!     // AtClient is used to call AT commands.
//!     // It is cloneable and can be safely shared across multiple threads.
//!     let at_client = at_client_handle.at_client();
//!
//!     // AtEventStream is used to receive all AT events.
//!     // Generally, there is no need to use it.
//!     // It is not cloneable, but it can be sent to another thread.
//!     let mut at_events = at_client_handle.at_event_stream()?;
//!     let h = tokio::spawn(async move {
//!         // AT incoming AT events, such as RING\r
//!         while let Some(at_events) = at_events.next().await {
//!             println!("{at_events:?}");
//!        }
//!     });
//!     // it's quite simple, the command derives the AtatCmd trait,
//!     // and the response derives the AtatResp trait.
//!     // The example below sends a simple 'AT' command and waits for OK.
//!     #[derive(Debug, PartialEq, AtatResp)]
//!     pub struct NoResponse;
//!
//!     #[derive(AtatCmd)]
//!     #[at_cmd("", NoResponse)] // An empty string ("") means just AT\r.
//!     struct AtRequest;
//!
//!     let resp = at_client.send(AtRequest).await?; // Send AT request
//!     assert_eq!(resp, NoResponse);
//!
//!     at_client_handle.shutdown().await?;
//!     Ok(())
//! }
//! ```
//! More advanced examples can be found in the documentation for [`AtClient::send`].
//!
//! For more documentation, please consult [`atat` 0.24.1](https://docs.rs/atat/0.24.1/atat/)
pub use atat::{AtatCmd, AtatLen, AtatResp, Error, InternalError, atat_derive, serde_at};

pub(crate) mod at_init;

use crate::SmResult;

use atat::asynch::{AtatClient, SimpleClient};
use atat::digest::ParseError;
use atat::{AtDigester, Parser};
use futures::Stream;
use futures::channel::mpsc::{self, Sender};
use futures::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::SerialStream;

use std::cell::RefCell;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::mpsc::sync_channel;

const EVT_CHANNEL_LEN: usize = 32;

/// Owns the AT engine and manages its lifetime.
///
/// `AtClientHandle` initializes the AT subsystem using the provided serial
/// device and keeps the background task responsible for communicating with
/// the modem alive.
///
/// Once initialized, it can create one or more [`AtClient`] instances through
/// [`AtClientHandle::at_client`]. These clients are lightweight, clonable handles
/// that can be used concurrently to send AT commands.
///
/// The handle must remain alive for all associated [`AtClient`] instances to
/// remain functional.
pub struct AtClientHandle {
    at_client: AtClient,
    at_events: Option<mpsc::Receiver<Vec<String>>>,
    at_thread: Option<std::thread::JoinHandle<()>>,
}

/// A lightweight handle for sending AT commands to the modem.
///
/// `AtClient` is created from an [`AtClientHandle`] and provides an asynchronous
/// interface for sending AT commands and receiving their responses.
///
/// Multiple `AtClient` instances may coexist and be used concurrently. All
/// requests are serialized internally by the underlying AT engine, ensuring
/// that commands are sent to the modem one at a time.
#[derive(Clone)]
pub struct AtClient {
    inner: Arc<AtClientInner>,
}

pub(crate) struct AtClientInner {
    sender: mpsc::UnboundedSender<AtRequest>,
}

pub(crate) struct TokioStream {
    stream: SerialStream,
}

impl embedded_io_async::ErrorType for TokioStream {
    type Error = std::io::Error;
}

impl embedded_io_async::Write for TokioStream {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.stream.write(buf).await
    }
    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.stream.flush().await
    }
}

impl embedded_io_async::Read for TokioStream {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.stream.read(buf).await
    }
}

pub(crate) struct NoUrcParser;

impl Parser for NoUrcParser {
    fn parse(buf: &[u8]) -> Result<(&[u8], usize), ParseError> {
        let s = String::from_utf8_lossy(buf)
            .lines()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<String>>();

        let _r = AT_EVT_SENDER.try_with(|f| {
            if let Some(sender) = &mut *f.borrow_mut() {
                drop(sender.try_send(s));
            }
        });
        Err(ParseError::NoMatch)
    }
}

#[rustfmt::skip]
type AtOperation = Box<dyn for<'a> FnOnce(&'a mut SimpleClient<TokioStream, AtDigester<NoUrcParser>>)
    -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> + Send>;

pub(crate) enum AtRequest {
    Operation(AtOperation),
    ShutDown,
}

/// A stream of all AT modem events.
///
/// `AtEventStream` receives every AT message from the modem,
/// whether it is an unsolicited result code (URC) or an
/// expected response to a command.
///
/// Events can be retrieved asynchronously with [`AtEventStream::next_at_events`]
/// or non-blockingly with [`AtEventStream::try_next_at_events`].
///
/// **Note**: The channel is bounded and can hold at most 32 pending events.
/// Additional events are dropped until space becomes available.
///
/// This structure is primarily intended for debugging. If you plan to use it for
/// more advanced purposes, it is recommended to integrate the atat crate directly
/// into your application rather than using the at-at feature of qmi-modem-manager.
///
/// This type also implements [`futures::Stream`], allowing it to be used with
/// standard asynchronous stream utilities.
pub struct AtEventStream(mpsc::Receiver<Vec<String>>);

thread_local! {
    static AT_EVT_SENDER: RefCell<Option<Sender<Vec<String>>>> = const { RefCell::new(None) }
}

impl AtClientHandle {
    /// Initializes the AT engine using the specified serial device.
    ///
    /// This function opens the serial device, starts the background task
    /// responsible for communicating with the modem, and returns an
    /// [`AtClientHandle`] managing its lifetime.
    ///
    /// Once initialized, one or more [`AtClient`] instances can be obtained
    /// with [`AtClientHandle::at_client`] and used to send AT commands.
    ///
    /// # Arguments
    ///
    /// * `at_tty_path` - Path to the modem's AT serial device (for example,
    ///   `/dev/ttyUSB3`).
    /// * `baud_rate` - The At serial device baud rate.
    ///
    /// # Errors
    ///
    /// Returns an error if the serial device cannot be opened or the AT
    /// engine fails to initialize.
    pub fn init(at_tty_path: impl AsRef<Path>, baud_rate: u32) -> SmResult<Self> {
        let (at_status_sender, at_status_receiver) =
            sync_channel::<SmResult<mpsc::UnboundedSender<AtRequest>>>(1);

        let (at_event_sender, at_event_receiver) = mpsc::channel(EVT_CHANNEL_LEN);

        let path = at_tty_path.as_ref().to_path_buf();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(crate::Error::TokioRuntimeError)?;

        let at_thread = std::thread::spawn(move || {
            AT_EVT_SENDER.set(Some(at_event_sender));
            at_init::at_init_thread(runtime, path, baud_rate, at_status_sender)
        });

        let engine_sender = at_status_receiver
            .recv()
            .map_err(|e| crate::Error::Other(e.to_string()))??;

        let at_client = AtClient {
            inner: Arc::new(AtClientInner {
                sender: engine_sender,
            }),
        };
        Ok(Self {
            at_client,
            at_events: Some(at_event_receiver),
            at_thread: Some(at_thread),
        })
    }

    /// Returns a clonable handle for sending AT commands.
    ///
    /// The returned [`AtClient`] is lightweight and may be cloned freely.
    /// Multiple clients can coexist and send commands concurrently. Requests
    /// are serialized internally by the AT engine.
    pub fn at_client(&self) -> AtClient {
        self.at_client.clone()
    }

    /// Returns the [`AtEventStream`] used to receive incoming AT responses.
    ///
    /// AtEventStream is not cloneable, so this method can only be called
    /// successfully once.
    pub fn at_event_stream(&mut self) -> SmResult<AtEventStream> {
        match self.at_events.take() {
            Some(events) => Ok(AtEventStream(events)),
            None => Err(crate::Error::EventsIteratorAlreadyTaken),
        }
    }

    /// Shuts down the AT engine and releases all associated resources.
    ///
    /// After shutdown, all existing [`AtClient`] instances become invalid and
    /// any attempt to send commands through them will fail.
    ///
    /// This method waits until the background task has terminated before
    /// returning.
    pub async fn shutdown(mut self) -> SmResult<()> {
        self.at_client
            .inner
            .sender
            .unbounded_send(AtRequest::ShutDown)?;
        if let Some(thread) = self.at_thread.take() {
            thread.join().map_err(|_| crate::Error::ThreadPanicked)?;
        }
        Ok(())
    }
}

impl AtClient {
    /// Sends an AT command to the modem and waits for its response.
    ///
    /// The command must implement [`atat::AtatCmd`], which defines both the
    /// command to send and the expected response type.
    ///
    /// Requests are queued and executed sequentially by the underlying AT
    /// engine, making it safe to use multiple [`AtClient`] instances
    /// concurrently.
    ///
    /// # Errors
    ///
    /// Returns an [`crate::Error::AtAtError`] if the command could not be sent, timed out,
    /// or if the modem returned an unexpected response.
    /// ```no_run
    /// # use futures::StreamExt;
    /// # use std::path::PathBuf;
    /// #
    /// # use qmi_sim_manager::atat::{self, AtClientHandle, atat_derive::{AtatCmd, AtatResp}};
    /// #
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// #     let at_client_handle = AtClientHandle::init(PathBuf::from("/dev/ttyUSB3"), 115200)?;
    /// #     let at_client = at_client_handle.at_client();
    /// #
    /// // it's quite simple, the command derives the AtatCmd trait,
    /// // and the response derives the AtatResp trait.
    /// #[derive(AtatResp)]
    /// pub struct NoResponse;
    ///
    /// // An optional timeout can be specified.
    /// #[derive(AtatCmd)]
    /// #[at_cmd("+CPOF", NoResponse, timeout_ms = 2000)]
    /// struct ShutdownModemRequest;
    ///
    /// let _resp = at_client.send(ShutdownModemRequest).await?;
    ///
    /// // A slightly more advanced example: here, the AT+CSQ command is sent,
    /// // and the returned values are parsed into the rssi and ber fields.
    /// #[derive(AtatResp)]
    /// pub struct CsqResponse {
    ///     #[at_arg(position = 0)]
    ///     pub rssi: u8,
    ///     #[at_arg(position = 1)]
    ///     pub ber: u8,
    /// }
    ///
    /// #[derive(AtatCmd)]
    /// #[at_cmd("+CSQ", CsqResponse)] // This generates the AT+CSQ\r command.
    /// pub struct CsqRequest;
    ///
    /// let _resp = at_client.send(CsqRequest).await?;
    /// #
    /// #     at_client_handle.shutdown().await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn send<Cmd>(&self, cmd: Cmd) -> SmResult<Cmd::Response>
    where
        Cmd: atat::AtatCmd + Send + Sync + 'static,
        Cmd::Response: Send + 'static,
    {
        let (reply_sender, reply_receiver) = tokio::sync::oneshot::channel();

        let operation: AtOperation = Box::new(move |simple_client| {
            Box::pin(async move {
                let result = simple_client.send(&cmd).await;

                if reply_sender.send(result).is_err() {
                    tracing::warn!("Unable to return AT command result");
                }
            })
        });

        self.inner
            .sender
            .unbounded_send(AtRequest::Operation(operation))?;

        reply_receiver
            .await
            .map_err(|e| crate::Error::Other(e.to_string()))?
            .map_err(crate::Error::AtAtError)
    }
}

impl Stream for AtEventStream {
    type Item = Vec<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        Pin::new(&mut this.0).poll_next(cx)
    }
}

impl AtEventStream {
    /// Waits for next AT events.
    ///
    /// Returns `None` if the AT event stream has been closed.
    pub async fn next_at_events(&mut self) -> Option<Vec<String>> {
        use futures::StreamExt;
        self.next().await
    }

    /// Attempts to retrieve the next pending AT events without blocking.
    ///
    /// Returns `Ok(None)` if no AT event is currently available.
    pub fn try_next_at_events(&mut self) -> SmResult<Option<Vec<String>>> {
        match self.0.try_recv() {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Closed) => Err(crate::Error::EngineStopped),
        }
    }
}
