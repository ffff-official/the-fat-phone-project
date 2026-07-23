use crate::auto::*;

use glib_boilerplates::Boilerplates;

/// Describes an application available on a UIM card.
///
/// A physical UIM card may expose one or more applications, such as SIM,
/// USIM, ISIM, or CSIM applications. This structure reports the current
/// state of one such application, including its application identifier,
/// personalization status, and PIN-related information.
///
/// Instances of this structure are returned as part of the card status
/// reported by the UIM service.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2 {
    /// The type of application exposed by the card.
    ///
    /// This identifies whether the application is, for example, a SIM,
    /// USIM, ISIM, or CSIM application.
    pub type_: UimCardApplicationType,
    /// The current initialization and availability state of the application.
    ///
    /// This indicates whether the application has been detected, is being
    /// initialized, is ready for use, or is in an error or blocked state.
    pub state: UimCardApplicationState,
    /// The current personalization state of the application.
    ///
    /// Personalization locks restrict the card or application to a specific
    /// network, network subset, service provider, corporate group, or other
    /// supported personalization category.
    pub personalization_state: UimCardApplicationPersonalizationState,
    /// The personalization feature currently associated with the application.
    ///
    /// This identifies the personalization category whose status is reported
    /// by [`Self::personalization_state`].
    pub personalization_feature: UimCardApplicationPersonalizationFeatureStatus,
    /// The number of remaining attempts to verify the personalization code.
    ///
    /// A value of zero may indicate that the personalization code is blocked
    /// and must be unblocked before further verification attempts can succeed.
    pub personalization_retries: u8,
    /// The number of remaining attempts to enter the personalization unblock
    /// code.
    pub personalization_unblock_retries: u8,
    /// Whether the universal PIN, or UPIN, replaces PIN1 for this application.
    ///
    /// When this value is `true`, the card-level UPIN is used instead of the
    /// application-specific PIN1.
    pub upin_replaces_pin1: bool,
    /// The current state of PIN1 for the application.
    ///
    /// PIN1 generally controls access to the main functionality of the
    /// application.
    pub pin1_state: UimPinState,
    /// The number of remaining PIN1 verification attempts.
    pub pin1_retries: u8,
    /// The number of remaining PUK1 unblock attempts.
    ///
    /// PUK1 is used to unblock PIN1 after the PIN1 retry counter reaches zero.
    pub puk1_retries: u8,
    /// The current state of PIN2 for the application.
    ///
    /// PIN2 protects additional or restricted SIM functionality and is
    /// independent of PIN1.
    pub pin2_state: UimPinState,
    /// The number of remaining PIN2 verification attempts.
    pub pin2_retries: u8,
    /// The number of remaining PUK2 unblock attempts.
    ///
    /// PUK2 is used to unblock PIN2 after the PIN2 retry counter reaches zero.
    pub puk2_retries: u8,
    /// The application identifier, or AID, of the application.
    ///
    /// The AID uniquely identifies an application available on the card. It is
    /// stored as raw bytes because it is a binary identifier rather than a
    /// UTF-8 string.
    pub application_identifier_value: Vec<u8>,
}

/// Describes the current status of a UIM card.
///
/// This structure represents one physical or logical card reported by the
/// `UIM Get Card Status` operation. It contains the overall state of the card,
/// information about its universal PIN, any card-level error, and the
/// applications exposed by the card.
///
/// A card may contain several applications, such as SIM, USIM, ISIM, or CSIM
/// applications. Their individual states and PIN information are available
/// through [`Self::applications`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct MessageUimGetCardStatusOutputCardStatusCardsElement {
    /// The current state of the card.
    ///
    /// This indicates whether the card is present and available, absent, or
    /// currently reporting an error.
    pub card_state: UimCardState,
    /// The current state of the universal PIN, or UPIN.
    ///
    /// The UPIN is a card-level PIN that may be shared by several applications
    /// on the same card. An application indicates whether it uses the UPIN
    /// instead of its own PIN1 through its `upin_replaces_pin1` field.
    pub upin_state: UimPinState,
    /// The number of remaining UPIN verification attempts.
    ///
    /// When this counter reaches zero, the UPIN is blocked and must be
    /// unblocked using the universal PUK.
    pub upin_retries: u8,
    /// The number of remaining universal PUK, or UPUK, unblock attempts.
    ///
    /// The UPUK is used to unblock the UPIN after the UPIN retry counter
    /// reaches zero.
    pub upuk_retries: u8,
    /// The error reported by the card.
    ///
    /// This field provides additional information when [`Self::card_state`]
    /// indicates that the card is in an error state. Its value should generally
    /// be ignored when the card is operating normally.
    pub error_code: UimCardError,
    /// The applications available on the card.
    ///
    /// A card may expose multiple applications, such as SIM, USIM, ISIM, or
    /// CSIM applications. Each element describes the state, personalization
    /// status, application identifier, and PIN information of one application.
    pub applications: Vec<MessageUimGetCardStatusOutputCardStatusCardsElementApplicationsElementV2>,
}

/// Describes a message routing rule configured in the modem.
///
/// Each instance defines how a specific category of incoming messages should
/// be handled. The rule matches a combination of message type and message
/// class, and specifies both the storage location and the action to perform
/// when a matching message is received.
///
/// The modem may expose multiple routing rules, each applying to a different
/// combination of message type and class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct MessageWmsGetRoutesOutputRouteListElement {
    /// Type of message matched by this routing rule.
    pub message_type: WmsMessageType,
    /// Message class matched by this routing rule.
    pub message_class: WmsMessageClass,
    /// Storage location associated with matching messages.
    pub storage: WmsStorageType,
    /// Action performed by the modem when a matching message is received.
    pub receipt_action: WmsReceiptAction,
}

/// Describes the current voice call state.
///
/// Each instance represents a single call reported by the modem in the
/// `all-call-status` indication. When multiple calls are active (for example,
/// during call waiting or a conference), one structure is provided for each
/// call.
///
/// The `id` field uniquely identifies the call and is used by other Voice
/// service operations such as answering or ending a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Boilerplates)]
#[boilerplates(prefix = "Qmi", source = "crate::ffi")]
pub struct IndicationVoiceAllCallStatusOutputCallInformationCall {
    /// Unique identifier assigned to the call by the modem.
    pub id: u8,
    /// Current state of the call.
    pub state: VoiceCallState,
    /// Type of the call.
    pub type_: VoiceCallType,
    /// Direction of the call (mobile-originated or mobile-terminated).
    pub direction: VoiceCallDirection,
    /// Radio technology used for the call.
    pub mode: VoiceCallMode,
    /// Indicates whether the call is part of a multiparty call.
    pub multipart_indicator: bool,
    /// Alternate Line Service associated with the call.
    pub als: VoiceAls,
}
