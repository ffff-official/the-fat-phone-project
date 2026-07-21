//! ```text
//! Protocol Identifier (TP-PID)
//! ```
#[derive(Debug)]
pub(crate) enum TpPid {
    /// No interworking, but SME-to-SME protocol : bit 8, 7, 6 set to zero. (mobile TO mobile)
    Normal,
    /// Exotics behaviors : Telefax, Teletex, ERMES etc...
    #[allow(unused)]
    Other(u8),
}

impl From<u8> for TpPid {
    fn from(v: u8) -> Self {
        match v {
            0x00 => Self::Normal,
            x => Self::Other(x),
        }
    }
}
