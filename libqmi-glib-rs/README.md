[![crates.io](https://img.shields.io/crates/v/libqmi-glib-rs.svg)](https://crates.io/crates/libqmi-glib-rs)
[![Docs.rs](https://docs.rs/libqmi-glib-rs/badge.svg)](https://docs.rs/libqmi-glib-rs)
[![License](https://img.shields.io/crates/l/libqmi-glib-rs.svg)](https://crates.io/crates/libqmi-glib-rs)
# libqmi-glib-rs

**libqmi-glib-rs** provides safe, asynchronous Rust bindings for **libqmi-glib**.

It allows applications to communicate with QMI-based cellular modems using idiomatic `async`/`await` while leveraging GLib's asynchronous APIs under the hood.

The library currently implements a subset of the QMI protocol, including device management, client creation, and common network service operations.

## Example

```rust
use libqmi_glib_rs::*;

use gio_sys::g_file_new_for_path;
use glib::MainLoop;
use gobject_sys::g_object_unref;

use std::ffi::CString;

fn main() {
    let ctx = glib::MainContext::default();
    let main_loop = MainLoop::new(Some(&ctx), false);
    let main_loop_clone = main_loop.clone();

    let path = CString::new("/dev/cdc-wdm1").unwrap();
    let file = unsafe { g_file_new_for_path(path.as_ptr()) };

    ctx.spawn_local(async move {
        let mut device = Device::new(file).await?;
        device.open().await?;

        let wds_client = WdsClient::new(&device).await?;
        let status = wds_client.get_packet_service_status().await?;

        println!("Packet service status: {:?}", status);

        wds_client.close().await?;
        device.close().await?;

        main_loop_clone.quit();

        Ok::<(), Box<dyn std::error::Error>>(())
    });

    main_loop.run();

    unsafe {
        g_object_unref(file as _);
    }
}
```

## Requirements

- Linux
- GLib
- libqmi-glib
- A QMI-compatible modem

## Status

This project is under active development and currently supports only a subset of the QMI protocol.