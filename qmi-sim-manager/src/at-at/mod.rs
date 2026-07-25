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
//! use qmi_sim_manager::at_at::*;
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Owns the service engine and is responsible for its initialization and shutdown.
//!     let at_client_handle = AtClientHandle::init(PathBuf::from("/dev/ttyUSB3"))?;
//!
//!     // AtClient is used to call AT commands.
//!     // It is cloneable and can be safely shared across multiple threads.
//!     let at_client = at_client_handle.at_client();
//!
//!     // it's quite simple, the command derives the AtatCmd trait,
//!     // and the response derives the AtatResp trait.
//!
//!     // The example below sends a simple AT command and waits for OK.
//!     #[derive(Debug, Clone, AtatResp)]
//!     pub struct NoResponse;
//!
//!     #[derive(Clone, AtatCmd)]
//!     #[at_cmd("", NoResponse)] // An empty string ("") means just AT\r.
//!     struct AtRequest;
//!
//!     let resp = at_client.send(&AtRequest).await?; // Send AT request
//!     assert_eq!(resp, Ok(NoResponse));
//!
//!     // An optional timeout can be specified.
//!     #[derive(Clone, AtatCmd)]
//!     #[at_cmd("+CPOF", NoResponse, timeout_ms = 2000)]
//!     struct ShutdownModemRequest;
//!
//!     // A slightly more advanced example: here, the AT+CSQ command is sent,
//!     // and the returned values are parsed into the rssi and ber fields.
//!     #[derive(Debug, Clone, AtatResp)]
//!     pub struct CsqResponse {
//!         #[at_arg(position = 0)]
//!         pub rssi: u8,
//!         #[at_arg(position = 1)]
//!         pub ber: u8,
//!     }
//!
//!     #[derive(Clone, AtatCmd)]
//!     #[at_cmd("+CSQ", CsqResponse)] // This generates the AT+CSQ\r command.
//!     pub struct CsqRequest;
//!
//!     at_client_handle.shutdown().await?;
//!     Ok(())
//! }
//! ```
//! For more documentation, please consult [`atat` 0.24.1](https://docs.rs/atat/0.24.1/atat/)
pub use atat::atat_derive::{AtatCmd, AtatResp};

pub(crate) mod at_engine;

use crate::{Error, SmResult};

use futures::channel::mpsc;

use std::path::Path;
use std::sync::Arc;

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
    inner: Arc<SmAtClientInner>,
}

struct SmAtClientInner {
    // engine_sender: mpsc::UnboundedSender<at_engine::Request>,
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
    ///
    /// # Errors
    ///
    /// Returns an error if the serial device cannot be opened or the AT
    /// engine fails to initialize.
    pub fn init(at_tty_path: impl AsRef<Path>) -> SmResult<Self> {
        let at_client = AtClient {
            inner: Arc::new(SmAtClientInner {}),
        };
        Ok(Self { at_client })
    }

    /// Returns a clonable handle for sending AT commands.
    ///
    /// The returned [`AtClient`] is lightweight and may be cloned freely.
    /// Multiple clients can coexist and send commands concurrently. Requests
    /// are serialized internally by the AT engine.
    pub fn at_client(&self) -> AtClient {
        self.at_client.clone()
    }

    /// Shuts down the AT engine and releases all associated resources.
    ///
    /// After shutdown, all existing [`AtClient`] instances become invalid and
    /// any attempt to send commands through them will fail.
    ///
    /// This method waits until the background task has terminated before
    /// returning.
    pub async fn shutdown(mut self) -> SmResult<()> {
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
    /// Returns an [`Error::AtAtError`] if the command could not be sent, timed out,
    /// or if the modem returned an unexpected response.
    pub async fn send<Cmd: atat::AtatCmd>(&mut self, cmd: &Cmd) -> SmResult<Cmd::Response> {
        Err(Error::AtAtError(atat::Error::Aborted))
    }
}
