# qmi-sim-manager

A Rust library for sending and receiving SMS and MMS messages through QMI-compatible cellular modems.

The library provides an asynchronous API for:

* Sending SMS messages.
* Receiving SMS messages and delivery reports.
* Receiving MMS notifications (WAP Push).
* Downloading MMS content.
* Sending MMS messages.

## Features

* Async API based on Tokio and Glib.
* SMS transmission.
* SMS reception.
* SMS delivery reports.
* MMS notification (WAP Push) reception.
* MMS download.
* MMS submission.

## Alpha status

This library is currently in an **alpha** stage. Its API and behavior may change before the first stable release.

At the moment, MMS sending is limited to image attachments.

## Example

```rust,no_run
use futures::stream::StreamExt;
use qmi_sim_manager::SmClientHandle;
use qmi_sim_manager::mms::{MmsContent, MmsContentType};

use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client_handle = SmClientHandle::init(PathBuf::from("/dev/cdc-wdm1"))?;

    let client = client_handle.client();

    client.send_sms("33612402244", "banane", true).await?;

    client.start_network("free").await?; // A mobile data connection is required to send an MMS through the MMSC.
    let response = client
        .send_mms(
            "http://mms.free.fr",
            vec![MmsContent {
                content: tokio::fs::read("bibendum.jpg").await?,
                content_type: Some(MmsContentType::ImageJpeg),
            }],
            None,
            &["33612402244"],
        )
        .await?;
    println!("{:?}", response);
    client.stop_network().await?;

    let mut events = client_handle.event_stream()?;
    let handler = tokio::spawn(async move {
        // Process incoming events, such as received SMS messages or ringing.
        while let Some(event) = events.next().await {
            println!("{event:?}");
        }
    });
    tokio::time::sleep(Duration::from_secs(60)).await; // Keep the client running for one minute before shutting it down.
    client_handle.shutdown().await?;
    handler.await?;
    Ok(())
}
```

# Why qmi_sim_manager?

`qmi_sim_manager` is an asynchronous Rust library that communicates directly with
QMI-compatible cellular modems. Unlike solutions based on ModemManager, it does
not rely on D-Bus or an external daemon, giving applications full control over
the modem lifecycle.

The library is designed for applications that need a simple, native Rust API
for SMS, MMS, and mobile network management.

## Features

* Native asynchronous Rust API based on `async`/`await`.
* Direct communication with QMI-compatible modems.
* No dependency on ModemManager or D-Bus.
* SMS transmission and reception.
* SMS delivery reports.
* MMS notification (WAP Push) handling.
* MMS download and decoding.
* MMS submission with image attachments.
* Event-driven API based on `futures::Stream`.
* Explicit control over mobile data connections.
* Suitable for embedded, kiosk, and headless Linux systems.

## Why not ModemManager?

ModemManager is an excellent choice for desktop environments and general modem
management. It provides a high-level abstraction over many modem technologies
and integrates well with the Linux desktop.

`qmi_sim_manager` has a different goal: it is intended for applications that
need direct, programmatic control over a QMI modem from Rust.

Compared to ModemManager, this library provides:

* A native asynchronous Rust API instead of a D-Bus interface.
* Direct communication with the modem through libqmi.
* No external daemon required at runtime.
* Fine-grained control over modem operations.
* Built-in support for MMS sending, downloading, and decoding.
* A simple event stream for incoming SMS, MMS notifications, and modem events.

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

This library is under active development. Future releases are expected to add support for additional modem features, including:

* Voice calls
* SIM card management
* Mobile network management
* GPS
* Additional QMI services

## License

This project is licensed under the terms of the MIT License.
