//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use dashmap::DashMap;
use futures::SinkExt;
use futures::StreamExt;
use tokio_util::codec::Framed;

use super::error::TestHarnessError;

#[derive(Default)]
pub(crate) struct Broker {
    name: String,
    connections: DashMap<String, ConnectionState>,
}

impl std::fmt::Debug for Broker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Broker")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Broker {
    pub fn new(name: String) -> Self {
        Self {
            name,
            connections: DashMap::new(),
        }
    }

    pub(crate) async fn connect(
        &self,
        client_name: String,
        connection: tokio::io::DuplexStream,
    ) -> Result<(), TestHarnessError> {
        let connection = Framed::new(connection, crate::codec::MqttPacketCodec);

        let state = ConnectionState { connection };

        self.connections.insert(client_name, state);
        Ok(())
    }

    pub(crate) async fn send(
        &mut self,
        client_name: &str,
        packet: mqtt_format::v5::packets::MqttPacket<'_>,
    ) -> Result<(), TestHarnessError> {
        let Some(mut r) = self.connections.get_mut(client_name) else {
            return Err(TestHarnessError::ClientNotFound(client_name.to_string()));
        };

        r.value_mut()
            .connection
            .send(packet)
            .await
            .map_err(TestHarnessError::Codec)
    }

    pub(crate) async fn wait_received(
        &self,
        client_name: &str,
        expected_packet: mqtt_format::v5::packets::MqttPacket<'_>,
    ) -> Result<(), TestHarnessError> {
        let Some(mut r) = self.connections.get_mut(client_name) else {
            return Err(TestHarnessError::ClientNotFound(client_name.to_string()));
        };

        let Some(next) = r.value_mut().connection.next().await else {
            return Err(TestHarnessError::StreamClosed(client_name.to_string()));
        };

        match next {
            Ok(packet) => {
                if *packet.get_packet() == expected_packet {
                    Ok(())
                } else {
                    Err(TestHarnessError::PacketNotExpected {
                        got: packet,
                    })
                }
            }
            Err(error) => Err(TestHarnessError::Codec(error)),
        }
    }
}

struct ConnectionState {
    connection: Framed<tokio::io::DuplexStream, crate::codec::MqttPacketCodec>,
}
