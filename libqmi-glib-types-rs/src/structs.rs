use crate::auto::*;

use glib_boilerplates::Boilerplates;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2 {
    pub type_: UimCardApplicationType,
    pub state: UimCardApplicationState,
    pub personalization_state: UimCardApplicationPersonalizationState,
    pub personalization_feature: UimCardApplicationPersonalizationFeatureStatus,
    pub personalization_retries: u8,
    pub personalization_unblock_retries: u8,
    pub upin_replaces_pin1: bool,
    pub pin1_state: UimPinState,
    pub pin1_retries: u8,
    pub puk1_retries: u8,
    pub pin2_state: UimPinState,
    pub pin2_retries: u8,
    pub puk2_retries: u8,
    pub application_identifier_value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct MessageUimGetCardStatusOutputCardStatusCardsElement {
    pub card_state: UimCardState,
    pub upin_state: UimPinState,
    pub upin_retries: u8,
    pub upuk_retries: u8,
    pub error_code: UimCardError,
    pub applications: Vec<MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct MessageWmsGetRoutesOutputRouteListElement {
    pub message_type: WmsMessageType,
    pub message_class: WmsMessageClass,
    pub storage: WmsStorageType,
    pub receipt_action: WmsReceiptAction,
}
