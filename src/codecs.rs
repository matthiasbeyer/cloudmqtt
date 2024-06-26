//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use mqtt_format::v5::packets::MqttPacket as FormatMqttPacket;
use tokio_util::codec::Decoder;
use tokio_util::codec::Encoder;
use winnow::Partial;
use yoke::Yoke;

use crate::packets::MqttPacket;
use crate::packets::MqttWriterError;

#[derive(Debug, thiserror::Error)]
pub enum MqttPacketCodecError {
    #[error("A codec error")]
    Io(#[from] std::io::Error),

    #[error("An error occured while writing to a buffer")]
    Writer(#[from] MqttWriterError),

    #[error("A protocol error occurred")]
    Protocol,

    #[error("Could not parse during decoding due to: {:?}", .0)]
    Parsing(winnow::error::ErrMode<winnow::error::ContextError>),
}

pub(crate) struct MqttPacketCodec;

impl Decoder for MqttPacketCodec {
    type Item = MqttPacket;

    type Error = MqttPacketCodecError;

    fn decode(
        &mut self,
        src: &mut tokio_util::bytes::BytesMut,
    ) -> Result<Option<Self::Item>, Self::Error> {
        // 1. Byte: FixedHeader
        // 2-5. Byte: Variable-Size

        if src.len() < 2 {
            src.reserve(2 - src.len());
            return Ok(None);
        }

        let remaining_length =
            match mqtt_format::v5::integers::parse_variable_u32(&mut Partial::new(&src[1..])) {
                Ok(size) => size as usize,
                Err(winnow::error::ErrMode::Incomplete(winnow::error::Needed::Size(needed))) => {
                    src.reserve(needed.into());
                    return Ok(None);
                }
                Err(winnow::error::ErrMode::Incomplete(winnow::error::Needed::Unknown)) => {
                    src.reserve(1);
                    return Ok(None);
                }
                _ => {
                    return Err(MqttPacketCodecError::Protocol);
                }
            };

        let total_packet_length = 1
            + mqtt_format::v5::integers::variable_u32_binary_size(remaining_length as u32) as usize
            + remaining_length;

        if src.len() < total_packet_length {
            src.reserve(total_packet_length - src.len());
            return Ok(None);
        }

        let cart = src.split_to(total_packet_length).freeze();

        let packet = Yoke::try_attach_to_cart(
            crate::packets::StableBytes(cart),
            |data| -> Result<_, MqttPacketCodecError> {
                FormatMqttPacket::parse_complete(data).map_err(MqttPacketCodecError::Parsing)
            },
        )?;

        Ok(Some(MqttPacket { packet }))
    }
}

impl Encoder<FormatMqttPacket<'_>> for MqttPacketCodec {
    type Error = MqttPacketCodecError;

    fn encode(
        &mut self,
        packet: FormatMqttPacket<'_>,
        dst: &mut tokio_util::bytes::BytesMut,
    ) -> Result<(), Self::Error> {
        let size = packet.binary_size() as usize;
        dst.reserve(size);

        let pre_size = dst.len();
        packet.write(&mut crate::packets::MqttWriter(dst))?;
        let total_written = dst.len() - pre_size;

        debug_assert_eq!(total_written, size, "Expected written bytes and actual written bytes differ! This is a bug for the {:?} packet type.", packet.get_kind());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use futures::SinkExt;
    use futures::StreamExt;
    use mqtt_format::v5::packets::connect::MConnect;
    use mqtt_format::v5::packets::pingreq::MPingreq;
    use mqtt_format::v5::packets::MqttPacket as FormatMqttPacket;
    use tokio_util::codec::Framed;
    use tokio_util::compat::TokioAsyncReadCompatExt;

    use super::MqttPacketCodec;
    use crate::transport::MqttConnection;

    #[tokio::test]
    async fn simple_test_codec() {
        let (client, server) = tokio::io::duplex(100);
        let mut framed_client =
            Framed::new(MqttConnection::Duplex(client.compat()), MqttPacketCodec);
        let mut framed_server =
            Framed::new(MqttConnection::Duplex(server.compat()), MqttPacketCodec);

        let packet = FormatMqttPacket::Pingreq(MPingreq);

        let sent_packet = packet.clone();
        tokio::spawn(async move {
            framed_client.send(sent_packet).await.unwrap();
        });
        let recv_packet = framed_server.next().await.unwrap().unwrap();

        assert_eq!(packet, *recv_packet.get());
    }

    #[tokio::test]
    async fn test_connect_codec() {
        let (client, server) = tokio::io::duplex(100);
        let mut framed_client =
            Framed::new(MqttConnection::Duplex(client.compat()), MqttPacketCodec);
        let mut framed_server =
            Framed::new(MqttConnection::Duplex(server.compat()), MqttPacketCodec);

        let packet = FormatMqttPacket::Connect(MConnect {
            client_identifier: "test",
            username: None,
            password: None,
            clean_start: false,
            will: None,
            properties: mqtt_format::v5::packets::connect::ConnectProperties::new(),
            keep_alive: 0,
        });

        let sent_packet = packet.clone();
        tokio::spawn(async move {
            framed_client.send(sent_packet.clone()).await.unwrap();
            framed_client.send(sent_packet).await.unwrap();
        });
        let recv_packet = framed_server.next().await.unwrap().unwrap();

        assert_eq!(packet, *recv_packet.get());
    }
}
