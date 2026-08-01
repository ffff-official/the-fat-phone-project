[![crates.io](https://img.shields.io/crates/v/libqmi-glib-rs.svg)](https://crates.io/crates/libqmi-glib-rs)
[![Docs.rs](https://docs.rs/libqmi-glib-rs/badge.svg)](https://docs.rs/libqmi-glib-rs)
[![License](https://img.shields.io/crates/l/libqmi-glib-rs.svg)](https://crates.io/crates/libqmi-glib-rs)
[![Requires libqmi](https://img.shields.io/badge/libqmi-required-ffc0cb)](https://www.freedesktop.org/software/libqmi/libqmi-glib/latest)
# libqmi-glib-rs

**libqmi-glib-rs** provides safe, asynchronous Rust bindings for **libqmi-glib**.

It allows Rust applications to communicate with QMI-compatible cellular modems using idiomatic `async`/`await` while relying on GLib's asynchronous infrastructure under the hood.

The library aims to provide a safe and ergonomic API while remaining close to the underlying QMI protocol.

## Features

The library currently provides support for a substantial subset of the QMI services exposed by **libqmi-glib**:

* **Device management**

  * Device creation and opening.
  * QMI client creation.
  * Resource management.

* **WDS (Wireless Data Service)**

  * Packet data connection management.
  * IPv4 network session management.
  * Current network settings.
  * Packet service status.

* **WMS (Wireless Messaging Service)**

  * Send SMS messages.
  * Receive incoming SMS indications.
  * Configure SMS routing.
  * Query and modify message routes.

* **NAS (Network Access Service)**

  * Signal quality.
  * Serving system information.
  * System selection preferences.
  * Band preferences.
  * Radio technology preferences.
  * Network registration information.
  * Various network configuration APIs.

* **DMS (Device Management Service)**

  * Device capabilities.
  * Device information.
  * Firmware and hardware information.
  * SIM and operating mode related operations.

* **VOICE**

  * Dial outgoing calls.
  * Answer incoming calls.
  * Hang up calls.
  * Burst DTMF.
  * Call status indications.

* **UIM (User Identity Module)**

  * Selected SIM management operations.
  * PIN management.
  * Slot management.

* **PDS (Position Determination Service)**

  * GPS service configuration.
  * Default tracking session management.

## Design goals

* Safe Rust wrappers around the libqmi-glib API.
* Idiomatic asynchronous API based on `async`/`await`.
* Automatic resource management.
* Consistent API across QMI services.
* Close mapping with the original libqmi-glib API.

## Example

```rust
use glib::MainLoop;
use libqmi_glib_rs::*;

fn main() {
    let ctx = glib::MainContext::default();
    let main_loop = MainLoop::new(Some(&ctx), false);
    let main_loop_clone = main_loop.clone();

    ctx.spawn_local(async move {
        let mut device = Device::new("/dev/cdc-wdm1").await?;
        device.open().await?;

        let wds_client = WdsClient::new(&device).await?;
        let status = wds_client.get_packet_service_status().await?;

        println!("Packet service status: {status:?}");

        wds_client.close().await?;
        device.close().await?;

        main_loop_clone.quit();

        Ok::<(), Box<dyn std::error::Error>>(())
    });
    main_loop.run();
}
```

## Requirements

* Linux
* GLib
* libqmi-glib
* A QMI-compatible cellular modem

## License

Licensed under either of

* Apache License, Version 2.0
* MIT License

at your option.