use std::fmt::Debug;

/// Error type returned by the library.
///
/// `QmiError` combines the human-readable error message reported by GLib with
/// additional information identifying the origin of the failure.
///
/// When available, the original GLib error domain and error code are preserved.
/// For QMI-specific errors, these values are used by the `Debug` implementation
/// to display the corresponding `ProtocolError` or `CoreError` variant.
///
/// The `context` field identifies the operation that produced the error,
/// making it easier to locate failures when debugging.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct QmiError {
    /// Human-readable error message.
    pub message: String,
    /// Name of the operation that produced the error.
    pub context: &'static str,
    /// Numeric error code reported by GLib, if available.
    pub code: Option<i32>,
    /// GLib error domain (`GQuark`), if available.
    pub domain: Option<u32>,
}

impl Debug for QmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use libqmi_glib_sys::{qmi_core_error_quark, qmi_protocol_error_quark};
        use libqmi_glib_types_rs::glib::translate::FromGlib;
        use libqmi_glib_types_rs::{CoreError, ProtocolError};

        if let (Some(domain), Some(code)) = (self.domain, self.code) {
            unsafe {
                let domain_name =
                    std::ffi::CStr::from_ptr(glib_sys::g_quark_to_string(domain)).to_string_lossy();
                if domain == qmi_protocol_error_quark() {
                    let protocol_error = ProtocolError::from_glib(code);
                    write!(
                        f,
                        "{}: [QMI_PROTOCOL_ERROR: {:?} ({})]",
                        self.context, protocol_error, code
                    )
                } else if domain == qmi_core_error_quark() {
                    let core_error = CoreError::from_glib(code);
                    write!(
                        f,
                        "{}: [QMI_CORE_ERROR: {:?} ({})]",
                        self.context, core_error, code
                    )
                } else {
                    write!(
                        f,
                        "{}: {} [{} ({})]",
                        self.context, self.message, domain_name, code
                    )
                }
            }
        } else {
            write!(f, "{}: {}", self.context, self.message)
        }
    }
}

#[doc(hidden)]
impl From<(*mut glib_sys::GError, &'static str)> for QmiError {
    fn from((mut error, context): (*mut glib_sys::GError, &'static str)) -> Self {
        if error.is_null() {
            return Self {
                message: "Unknown GLib error".into(),
                context,
                code: None,
                domain: None,
            };
        }

        unsafe {
            let ret = Self {
                message: std::ffi::CStr::from_ptr((*error).message)
                    .to_string_lossy()
                    .into_owned(),
                context,
                code: Some((*error).code),
                domain: Some((*error).domain),
            };

            glib_sys::g_clear_error(&mut error);
            ret
        }
    }
}

impl std::fmt::Display for QmiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

impl QmiError {
    pub(crate) fn new(message: impl AsRef<str>, context: &'static str) -> Self {
        Self {
            message: message.as_ref().to_string(),
            context,
            code: None,
            domain: None,
        }
    }
}

impl std::error::Error for QmiError {}

impl QmiError {
    pub(crate) fn cb_droped(context: &'static str) -> Self {
        Self {
            message: "cb droped".to_string(),
            context,
            code: None,
            domain: None,
        }
    }
}
