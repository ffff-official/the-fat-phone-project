use crate::QmiResult;

pub(crate) fn cannot_send<T: std::fmt::Debug>(result: &QmiResult<T>) {
    tracing::error!("Cannot send : {:?}", result);
}

pub(crate) fn tracing_debug_hex(source: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    use std::fmt::Write;

    let mut hex = String::new();
    writeln!(&mut hex, "source raw values :")?;
    for b in source {
        write!(&mut hex, "{:02X} ", b)?;
    }
    hex.pop();

    tracing::debug!("{hex}");
    Ok(())
}

pub(crate) unsafe fn c_char_to_string(value: *const std::ffi::c_char) -> String {
    std::ffi::CStr::from_ptr(value)
        .to_string_lossy()
        .into_owned()
}
