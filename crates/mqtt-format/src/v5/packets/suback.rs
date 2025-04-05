//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use winnow::Bytes;
use winnow::Parser;
use winnow::error::ParserError;

use crate::v5::MResult;
use crate::v5::variable_header::PacketIdentifier;
use crate::v5::variable_header::ReasonString;
use crate::v5::variable_header::UserProperties;
use crate::v5::write::WResult;
use crate::v5::write::WriteMqttPacket;

crate::v5::reason_code::make_combined_reason_code! {
    pub enum SubackReasonCode {
        GrantedQoS0 = crate::v5::reason_code::GrantedQoS0,
        GrantedQoS1 = crate::v5::reason_code::GrantedQoS1,
        GrantedQoS2 = crate::v5::reason_code::GrantedQoS2,
        ImplementationSpecificError = crate::v5::reason_code::ImplementationSpecificError,
        NotAuthorized = crate::v5::reason_code::NotAuthorized,
        PacketIdentifierInUse = crate::v5::reason_code::PacketIdentifierInUse,
        QuotaExceeded = crate::v5::reason_code::QuotaExceeded,
        SharedSubscriptionsNotSupported = crate::v5::reason_code::SharedSubscriptionsNotSupported,
        SubscriptionIdentifiersNotSupported = crate::v5::reason_code::SubscriptionIdentifiersNotSupported,
        TopicFilterInvalid = crate::v5::reason_code::TopicFilterInvalid,
        UnspecifiedError = crate::v5::reason_code::UnspecifiedError,
        WildcardSubscriptionsNotSupported = crate::v5::reason_code::WildcardSubscriptionsNotSupported,
    }
}

impl SubackReasonCode {
    /// Return Some(_) if self is a "Granted" reason code, otherwise None
    pub fn granted(&self) -> Option<Self> {
        match self {
            SubackReasonCode::GrantedQoS0
            | SubackReasonCode::GrantedQoS1
            | SubackReasonCode::GrantedQoS2 => Some(*self),
            _ => None,
        }
    }
}

crate::v5::properties::define_properties! {
    packet_type: MSuback,
    anker: "_Toc3901174",
    pub struct SubackProperties<'i> {
        (anker: "_Toc3901175")
        reason_string: ReasonString<'i>,

        (anker: "_Toc3901176")
        user_properties: UserProperties<'i>,
    }
}

#[cfg_attr(feature = "yoke", derive(yoke::Yokeable))]
#[derive(Clone, Debug, PartialEq)]
#[doc = crate::v5::util::md_speclink!("_Toc3901171")]
pub struct MSuback<'i> {
    pub packet_identifier: PacketIdentifier,
    pub properties: SubackProperties<'i>,
    pub reasons: &'i [SubackReasonCode],
}

impl<'i> MSuback<'i> {
    pub fn parse(input: &mut &'i Bytes) -> MResult<Self> {
        winnow::combinator::trace("MSuback", |input: &mut &'i Bytes| {
            let packet_identifier = PacketIdentifier::parse(input)?;
            let properties = SubackProperties::parse(input)?;

            // Verify that the payload only contains valid reason codes
            let payload: &[u8] = winnow::combinator::repeat_till::<_, _, (), _, _, _, _>(
                0..,
                SubackReasonCode::parse,
                winnow::combinator::eof,
            )
            .take()
            .parse_next(input)?;

            let reasons: &[SubackReasonCode] =
                bytemuck::checked::try_cast_slice(payload).map_err(|_e| {
                    winnow::error::ErrMode::Cut(winnow::error::ContextError::from_input(input))
                })?;

            Ok(Self {
                packet_identifier,
                properties,
                reasons,
            })
        })
        .parse_next(input)
    }

    pub fn binary_size(&self) -> u32 {
        self.packet_identifier.binary_size()
            + self.reasons.len() as u32
            + self.properties.binary_size()
    }

    pub fn write<W: WriteMqttPacket>(&self, buffer: &mut W) -> WResult<W> {
        self.packet_identifier.write(buffer)?;
        self.properties.write(buffer)?;

        let reasons: &[u8] = bytemuck::cast_slice(self.reasons);

        buffer.write_slice(reasons)
    }
}

#[cfg(test)]
mod test {
    use crate::v5::packets::suback::MSuback;
    use crate::v5::packets::suback::SubackProperties;
    use crate::v5::packets::suback::SubackReasonCode;
    use crate::v5::variable_header::PacketIdentifier;
    use crate::v5::variable_header::ReasonString;
    use crate::v5::variable_header::UserProperties;

    #[test]
    fn test_roundtrip_suback_no_props() {
        crate::v5::test::make_roundtrip_test!(MSuback {
            packet_identifier: PacketIdentifier(core::num::NonZeroU16::new(17).unwrap()),
            reasons: &[SubackReasonCode::GrantedQoS0],
            properties: SubackProperties {
                reason_string: None,
                user_properties: None,
            },
        });
    }

    #[test]
    fn test_roundtrip_suback_props() {
        crate::v5::test::make_roundtrip_test!(MSuback {
            packet_identifier: PacketIdentifier(core::num::NonZeroU16::new(17).unwrap()),
            reasons: &[SubackReasonCode::GrantedQoS0],
            properties: SubackProperties {
                reason_string: Some(ReasonString("sgjdhsbgjsghb")),
                user_properties: Some(UserProperties(&[0x0, 0x1, b'f', 0x0, 0x2, b'h', b'j'])),
            },
        });
    }
}
