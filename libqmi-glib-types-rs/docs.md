<!-- file * -->
<!-- const CID_BROADCAST -->
A symbol specifying the broadcast CID.
<!-- const CID_NONE -->
A symbol specifying a special CID value that references no CID.
<!-- static CLIENT_CID -->
Symbol defining the `QmiClient:client-cid` property.
<!-- static CLIENT_DEVICE -->
Symbol defining the `QmiClient:client-device` property.
<!-- static CLIENT_SERVICE -->
Symbol defining the `QmiClient:client-service` property.
<!-- static CLIENT_VALID -->
Symbol defining the `QmiClient:client-valid` property.
<!-- static CLIENT_VERSION_MAJOR -->
Symbol defining the `QmiClient:client-version-major` property.
<!-- static CLIENT_VERSION_MINOR -->
Symbol defining the `QmiClient:client-version-minor` property.
<!-- static DBUS_ERROR_PREFIX -->
Symbol defining the common string prefix used for all libqmi errors in DBus.
<!-- static DEVICE_CONSECUTIVE_TIMEOUTS -->
Symbol defining the `QmiDevice:device-consecutive-timeouts` property.
<!-- static DEVICE_FILE -->
Symbol defining the `QmiDevice:device-file` property.
<!-- const DEVICE_MUX_ID_MAX -->
Symbol defining the maximum supported mux id.
<!-- const DEVICE_MUX_ID_MIN -->
Symbol defining the minimum supported mux id.
<!-- const DEVICE_MUX_ID_UNBOUND -->
Symbol defining the mux id for an unbound interface. This value is also used
to indicate an invalid mux id.
<!-- static DEVICE_NODE -->
Symbol defining the `QmiDevice:device-node` property.

`<note>``<para>`
This property is only available when the library is built with QRTR support.
`</para>``</note>`
<!-- static DEVICE_NO_FILE_CHECK -->
Symbol defining the `QmiDevice:device-no-file-check` property.
<!-- static DEVICE_PROXY_PATH -->
Symbol defining the `QmiDevice:device-proxy-path` property.
<!-- static DEVICE_SIGNAL_INDICATION -->
Symbol defining the `QmiDevice::indication` signal.
<!-- static DEVICE_SIGNAL_REMOVED -->
Symbol defining the `QmiDevice::device-removed` signal.
<!-- static DEVICE_WWAN_IFACE -->
Symbol defining the `QmiDevice:device-wwan-iface` property.
<!-- const MESSAGE_QMUX_MARKER -->
First byte of every QMI QMUX message.
<!-- const MESSAGE_QRTR_MARKER -->
Fake header added by libqmi to re-use existing QMUX message parsers for QRTR messages.
QRTR QMI services with a service ID > 0xFF use this fake header where the service ID
is set to 16 bits instead of 8 bits. This header has no purpose outside of libqmi
and is never send to the actual device implementing these QMI services.
<!-- const MESSAGE_VENDOR_GENERIC -->
Generic vendor id (0x0000).
<!-- static PROXY_N_CLIENTS -->
Symbol defining the `QmiProxy:qmi-proxy-n-clients` property.
<!-- static PROXY_SOCKET_PATH -->
Symbol defining the default abstract socket name where the `QmiProxy` will listen.
<!-- const WDS_RATE_UNAVAILABLE -->
The TX/RX rate information is unavailable or cannot be measured.
<!-- enum LocSessionStatus::variant EngineLocked -->
Engine is locked.
