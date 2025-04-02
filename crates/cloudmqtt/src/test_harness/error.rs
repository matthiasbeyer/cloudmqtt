//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#[derive(Debug, thiserror::Error)]
#[allow(clippy::large_enum_variant)]
pub enum TestHarnessError {
    #[error("Broker '{}' not found", .0)]
    BrokerNotFound(String),

    #[error("Client '{}' not found", .0)]
    ClientNotFound(String),

    #[error("Internal channel error")]
    Channel,

    #[error("Client errored")]
    Client(#[source] crate::error::Error),

    #[error("Codec error")]
    Codec(#[source] crate::codec::MqttPacketCodecError),

    #[error("Stream for '{}' closed", .0)]
    StreamClosed(String),

    #[error("Received not expected packet: '{:?}'", .got)]
    PacketNotExpected { got: Box<crate::codec::MqttPacket> },

    #[error("Unexpected client identifier: got {}, expected {}", .got, .expected)]
    UnexpectedClientIdentifier { got: String, expected: String },

    #[error("Packet {} not received", .0)]
    PacketNotReceived(String),

    #[error("Unexpected topic, expected: {}, found: {}", .expected, .found)]
    UnexpectedTopic { expected: String, found: String },

    #[error("Unexpected packet type: expected {}, found {}", .expected, .found.mqtt_packet_type_name())]
    UnexpectedPacketType {
        expected: String,
        found: crate::codec::MqttPacket,
    },

    #[error("Packet payload is not valid UTF8")]
    PayloadNotUtf8,
}

/// Helper trait for error construction
trait MqttPacketTypeName {
    fn mqtt_packet_type_name(&self) -> &'static str;
}

impl MqttPacketTypeName for crate::codec::MqttPacket {
    fn mqtt_packet_type_name(&self) -> &'static str {
        match self.get_packet() {
            mqtt_format::v5::packets::MqttPacket::Auth(_) => "Auth",
            mqtt_format::v5::packets::MqttPacket::Connack(_) => "Connack",
            mqtt_format::v5::packets::MqttPacket::Connect(_) => "Connect",
            mqtt_format::v5::packets::MqttPacket::Disconnect(_) => "Disconnect",
            mqtt_format::v5::packets::MqttPacket::Pingreq(_) => "Pingreq",
            mqtt_format::v5::packets::MqttPacket::Pingresp(_) => "Pingresp",
            mqtt_format::v5::packets::MqttPacket::Puback(_) => "Puback",
            mqtt_format::v5::packets::MqttPacket::Pubcomp(_) => "Pubcomp",
            mqtt_format::v5::packets::MqttPacket::Publish(_) => "Publish",
            mqtt_format::v5::packets::MqttPacket::Pubrec(_) => "Pubrec",
            mqtt_format::v5::packets::MqttPacket::Pubrel(_) => "Pubrel",
            mqtt_format::v5::packets::MqttPacket::Suback(_) => "Suback",
            mqtt_format::v5::packets::MqttPacket::Subscribe(_) => "Subscribe",
            mqtt_format::v5::packets::MqttPacket::Unsuback(_) => "Unsuback",
            mqtt_format::v5::packets::MqttPacket::Unsubscribe(_) => "Unsubscribe",
        }
    }
}
