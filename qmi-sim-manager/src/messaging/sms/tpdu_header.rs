use bitfields::{bitfield, bitflag};

#[bitfield(u8)]
pub(crate) struct TpduHeaderSmsDeliver {
    /// Message type indicator. Bits no 1 and 0 are both set to 0 to indicate that this PDU is an SMS-DELIVER
    #[bits(2, default = Mti::SMS_DELIVER)]
    pub tp_mti: Mti,
    /// More messages to send : This bit is set to 0 if there are more messages to send
    #[bits(1)]
    pub tp_mms: u8,
    #[bits(2, default = 0x00)]
    _unused: u8,
    /// Status report indication : This bit is set to 1 if a status report is going to be returned to the SME
    #[bits(1)]
    pub tp_sri: u8,
    /// User data header indicator : This bit is set to 1 if the User Data field starts with a header : cf UDH
    #[bits(1)]
    pub tp_udhi: u8,
    /// Reply path. Parameter indicating that reply path exists.
    #[bits(1)]
    pub tp_rp: u8,
}

#[bitfield(u8)]
pub(crate) struct TpduHeaderSmsSubmit {
    /// Message type indicator.
    #[bits(2, default = Mti::SMS_SUBMIT)]
    pub tp_mti: Mti,
    /// Reject Duplicates
    #[bits(1)]
    pub tp_rd: u8,
    /// TP-VP field format
    #[bits(2, default = Vpf::RELATIVE)]
    pub tp_vpf: Vpf,
    /// Status Report Request : This bit is set to 1 if a status report is going to be returned to the SME
    #[bits(1)]
    pub tp_srr: u8,
    /// User data header indicator : This bit is set to 1 if the User Data field starts with a header : cf UDH
    #[bits(1)]
    pub tp_udhi: u8,
    /// Reply path. Parameter indicating that reply path exists.
    #[bits(1)]
    pub tp_rp: u8,
}

/// bit:   7      6      5      4      3      2      1      0
///      +------+------+------+------+------+------+------+------+
///      | TP-RP|UDHI |  LP  | SRQ   |unused| MMS  | MTI1 | MTI0 |
///      +------+------+------+------+------+------+------+------+
#[bitfield(u8)]
pub(crate) struct TpduHeaderSmsStatusReport {
    /// Message type indicator.
    #[bits(2, default = Mti::SMS_STATUS_REPORT)]
    pub tp_mti: Mti,
    /// More messages to send : This bit is set to 0 if there are more messages to send
    #[bits(1)]
    pub tp_mms: u8,
    #[bits(1, default = 0)]
    pub _unused: u8,
    /// Status Report Qualifier : 0 => concerns SMS-SUBMIT 1 => concerns SMS-COMMAND
    #[bits(1, default = 0)]
    pub tp_srq: u8,
    /// Loop Prevention
    #[bits(1)]
    pub tp_lp: u8,
    /// User Data Header Indicator.
    #[bits(1)]
    pub tp_udki: u8,
    /// Reply Path.
    #[bits(1)]
    pub tp_rp: u8,
}

/// TPDU	             TP-MTI
/// --------------------------
/// SMS-DELIVER        |  00b
/// SMS-SUBMIT         |  01b
/// SMS-STATUS-REPORT  |  10b
/// SMS-SUBMIT-REPORT  |  01b (direction SC → MS)
#[derive(Debug)]
#[bitflag(u8)]
#[allow(non_camel_case_types)]
pub(crate) enum Mti {
    #[base]
    SMS_DELIVER = 0b00,
    SMS_SUBMIT = 0b01, // OR AND _REPORT
    SMS_STATUS_REPORT = 0b10,
    RESERVED = 0b11,
}

#[derive(Debug)]
#[bitflag(u8)]
#[allow(non_camel_case_types)]
pub(crate) enum Vpf {
    UNKNOWN = 0b00,
    UPGRADED = 0b01, // Woot ?
    #[base]
    RELATIVE = 0b10,
    ABSOLUTE = 0b11,
}
