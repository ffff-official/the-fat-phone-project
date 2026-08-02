//! **libqmi-glib-rs provides safe, asynchronous Rust bindings for libqmi-glib.**
//!
//! It allows applications to communicate with QMI-based cellular modems using
//! idiomatic async/await while leveraging GLib's asynchronous APIs under the
//! hood.
//!
//! The library exposes a safe, high-level API covering a substantial subset of
//! libqmi-glib services, including:
//!
//! - **Device management**, including device creation and lifecycle handling.
//! - **WDS (Wireless Data Service)** for packet data connections and network
//!   configuration.
//! - **WMS (Wireless Messaging Service)** for sending SMS messages, receiving
//!   SMS indications, and managing message routes.
//! - **NAS (Network Access Service)** for network registration, signal quality,
//!   system selection preferences, and radio information.
//! - **DMS (Device Management Service)** for querying modem capabilities and
//!   device information.
//! - **VOICE** for placing, answering, terminating, and monitoring voice calls
//!   through asynchronous call status indications.
//! - **UIM (User Identity Module)** for selected SIM card management
//!   operations.
//! - **PDS (Position Determination Service)** for selected GNSS/GPS
//!   configuration features.
//!
//! The API follows Rust idioms while remaining close to the underlying QMI
//! protocol. Most QMI requests are exposed as asynchronous methods, and common
//! request patterns are implemented through reusable abstractions, providing a
//! consistent interface across services.
//! ```no_run
//! use libqmi_glib_rs::*;
//!
//! use glib::MainLoop;
//!
//! fn main() {
//!     let ctx = glib::MainContext::default();
//!     let main_loop = MainLoop::new(Some(&ctx), false);
//!     let main_loop_clone = main_loop.clone();
//!
//!     ctx.spawn_local(async move {
//!         let mut device = Device::new("/dev/cdc-wdm1").await?;
//!         device.open().await?;
//!
//!         let wds_client = WdsClient::new(&device).await?;
//!         let status = wds_client.get_packet_service_status().await?;
//!
//!         println!("Packet service status: {:?}", status);
//!
//!         wds_client.close().await?;
//!         device.close().await?;
//!
//!         main_loop_clone.quit();
//!
//!         Ok::<(), Box<dyn std::error::Error>>(())
//!     });
//!     main_loop.run();
//! }
//! ```
#![cfg_attr(docsrs, feature(doc_cfg))]
#![allow(unsafe_op_in_unsafe_fn)] // Warnings on unsafe_op_in_unsafe_fn is a shitty idea for a crate of this kind.
#![deny(missing_docs)]

mod error;
#[doc(inline)]
pub use error::QmiError;

/// Convenient alias for the result type returned by this crate.
///
/// The `Ok` variant contains the requested value, while the `Err` variant
/// contains a [`QmiError`].
pub type QmiResult<T> = Result<T, QmiError>;

mod helpers;
use helpers::*;

#[macro_use]
mod macros;
use macros::__macro_support;

/// Public definition for the Device.
pub mod device;
/// Public types, enums, flags, and other definitions for the DMS service.
pub mod dms;
/// Public types, enums, flags, and other definitions for the NAS service.
pub mod nas;
/// Public types, enums, flags, and other definitions for the PDS service.
pub mod pds;
/// Public types, enums, flags, and other definitions for the UIM service.
pub mod uim;
#[cfg(feature = "v1_14")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
/// Public types, enums, flags, and other definitions for the VOICE service.
pub mod voice;
/// Public types, enums, flags, and other definitions for the WDS service.
pub mod wds;
/// Public types, enums, flags, and other definitions for the WMS service.
pub mod wms;

mod sys {
    pub(crate) use libqmi_glib_sys::*;
}
/// Definitions from libqmi-glib-types-rs.
pub mod types {
    pub use libqmi_glib_types_rs::*;
}

#[doc(inline)]
pub use device::Device;
#[doc(inline)]
pub use dms::DmsClient;
#[doc(inline)]
pub use nas::NasClient;
#[doc(inline)]
pub use pds::PdsClient;
#[doc(inline)]
pub use uim::UimClient;
#[cfg(feature = "v1_14")]
#[cfg_attr(docsrs, doc(cfg(feature = "v1_14")))]
#[doc(inline)]
pub use voice::VoiceClient;
#[doc(inline)]
pub use wds::WdsClient;
#[doc(inline)]
pub use wms::WmsClient;

const OPERATION_TIMEOUT: u32 = 5;
