[![crates.io](https://img.shields.io/crates/v/qmi-sim-manager.svg)](https://crates.io/crates/qmi-sim-manager)
[![Docs.rs](https://docs.rs/qmi-sim-manager/badge.svg)](https://docs.rs/qmi-sim-manager)
[![License](https://img.shields.io/crates/l/qmi-sim-manager.svg)](https://crates.io/crates/qmi-sim-manager)
[![Requires libqmi](https://img.shields.io/badge/libqmi-%E2%89%A51.32-ffc0cb)](https://www.freedesktop.org/software/libqmi/libqmi-glib/latest)
# qmi-sim-manager

A Rust library for interacting with QMI-compatible cellular modems.

The library provides an asynchronous API for:

* Placing, answering, and terminating voice calls.
* Sending and receiving SMS messages.
* Downloading and sending MMS messages.
* AT command basic support. (via feature at-at)
* Receiving SMS delivery reports.
* Receiving MMS notifications (WAP Push).
* Managing packet data connections.
* Monitoring network registration and connectivity.
* Managing UIM (SIM) cards and applications.
* Receiving asynchronous modem events.

## Features

* Native asynchronous Rust API based on `async`/`await`.
* Powered by Tokio and GLib.
* Direct communication with QMI-compatible modems.
* No dependency on ModemManager or D-Bus.
* Event-driven API based on `futures::Stream`.
* Suitable for embedded, kiosk, and headless Linux systems.

## Beta status

The public API is considered stable and is not expected to undergo significant changes before the first stable release.

The library is still under active development, and some features are incomplete and can receive improvements based on feedback.

At the moment:

* SIM/UIM features are still limited.
* The library cannot currently configure the default gateway.

## Example

```rust,no_run
use futures::stream::StreamExt;
use qmi_sim_manager::ClientHandle;
use qmi_sim_manager::mms::{MmsContent, WspContentType};
use qmi_sim_manager::mms::WspWellKnownContentType::ImageJpeg;

use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client_handle = ClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    client.send_sms("33612402244", "banane", true).await?;

    client.start_network("free").await?; // A mobile data connection is required to send an MMS through the MMSC.
    let response = client
        .send_mms(
            "http://mms.free.fr",
            vec![MmsContent {
                content: tokio::fs::read("bibendum.jpg").await?,
                content_type: content_type: WspContentType::WspWellKnownContentType(ImageJpeg),
            }],
            None,
            &["33612402244"],
        )
        .await?;

    println!("{response:?}");

    client.stop_network().await?;

    let mut events = client_handle.event_stream()?;
    let handler = tokio::spawn(async move {
        // Process incoming modem events.
        while let Some(event) = events.next().await {
            println!("{event:?}");
        }
    });

    tokio::time::sleep(Duration::from_secs(60)).await;

    client_handle.shutdown().await?;
    handler.await?;

    Ok(())
}
```

## Why qmi-sim-manager?

`qmi-sim-manager` is an asynchronous Rust library that communicates directly with QMI-compatible cellular modems through **libqmi**.

Unlike solutions based on ModemManager, it does not rely on D-Bus or an external daemon, giving applications full control over the modem lifecycle.

The library is intended for applications requiring a native Rust API for messaging, mobile networking, voice calls, and SIM management while remaining lightweight and suitable for embedded systems.

## Why not ModemManager?

ModemManager is an excellent choice for desktop environments and general modem management.

`qmi-sim-manager` targets a different use case: applications that require direct programmatic access to QMI services from Rust.

Compared to ModemManager, this library provides:

* A native asynchronous Rust API.
* Direct communication with the modem through libqmi.
* No external daemon required at runtime.
* Fine-grained control over modem operations.
* Built-in MMS support.
* Integrated voice call support.
* Event-driven programming through `futures::Stream`.
* Explicit control over packet data connections.

# Linux setup

Before using this library, your Linux system must be configured to allow
non-root access to the QMI device and its associated network interface.

## Disable ModemManager

This library requires exclusive access to the QMI modem. If ModemManager is
running, it may interfere with the communication by opening the same QMI device.

To stop it temporarily:

```sh
sudo systemctl stop ModemManager
```

To disable it permanently:

```sh
sudo systemctl disable --now ModemManager
```

You can verify that it is no longer running:

```sh
systemctl status ModemManager
```

## Install libqmi

Install the development package:

```sh
sudo apt install libqmi-glib-dev
```

## Grant access to the QMI device

Add your user to the `dialout` group:

```sh
sudo usermod -aG dialout $USER
```

Create the following udev rule in `/etc/udev/rules.d/90-qmi.rules`:

```text
KERNEL=="cdc-wdm[0-9]*", MODE="0660", GROUP="dialout"
```

## Allow non-root access to RAW-IP mode

Create the `qmi` group and add your user to it:

```sh
sudo groupadd -f qmi
sudo usermod -aG qmi $USER
```

Create the following udev rule in `/etc/udev/rules.d/91-qmi.rules`:

```text
ACTION=="add|change", SUBSYSTEM=="net", KERNEL=="wwan*", \
RUN+="/bin/chgrp qmi /sys/class/net/%k/qmi/raw_ip", \
RUN+="/bin/chmod 0660 /sys/class/net/%k/qmi/raw_ip"
```

Reload the udev rules:

```sh
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Verify that the permissions have been applied:

```sh
ls -l /sys/class/net/${your_wwan_device}/qmi/raw_ip
```

The output should be similar to:

```text
-rw-rw---- 1 root qmi ... /sys/class/net/${your_wwan_device}/qmi/raw_ip
```

At this point, it should be possible to enable RAW-IP mode as a regular user:

```sh
echo Y > /sys/class/net/${your_wwan_device}/qmi/raw_ip
```

The following command should also work without root privileges:

```sh
qmicli -d /dev/${your_qmi_device} --set-expected-data-format=raw-ip
```

## Grant network administration capability

The application must be granted the `CAP_NET_ADMIN` Linux capability in order
to configure the mobile network interface without requiring root privileges.

Replace `$bin` with the path to your executable:

```sh
sudo setcap cap_net_admin+ep "$bin"
```

For example:

```sh
sudo setcap cap_net_admin+ep target/release/my_application
```

You can verify that the capability has been applied with:

```sh
sudo getcap target/release/my_application
```

The output should be similar to:

```text
target/release/my_application cap_net_admin=ep
```

## Future development

This library is under active development.

Future releases are expected to expand support for additional modem features, including:

* Extended SIM/UIM management.
* GPS and positioning services.
* Additional QMI services.
* Improved MMS capabilities.
* Additional voice features.

## License

This project is licensed under the terms of the MIT License.
