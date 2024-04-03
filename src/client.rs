//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::num::NonZeroU16;
use std::sync::Arc;

use futures::lock::Mutex;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use mqtt_format::v5::integers::VARIABLE_INTEGER_MAX;
use mqtt_format::v5::packets::publish::MPublish;
use tokio_util::codec::FramedRead;
use tokio_util::codec::FramedWrite;
use tracing::Instrument;

use crate::bytes::MqttBytes;
use crate::client_identifier::ProposedClientIdentifier;
use crate::codecs::MqttPacketCodec;
use crate::codecs::MqttPacketCodecError;
use crate::keep_alive::KeepAlive;
use crate::packets::connack::ConnackPropertiesView;
use crate::payload::MqttPayload;
use crate::qos::QualityOfService;
use crate::string::MqttString;
use crate::transport::MqttConnectTransport;
use crate::transport::MqttConnection;

#[derive(Debug, PartialEq, Eq)]
pub enum CleanStart {
    No,
    Yes,
}

impl CleanStart {
    pub fn as_bool(&self) -> bool {
        match self {
            CleanStart::No => false,
            CleanStart::Yes => true,
        }
    }
}

#[derive(typed_builder::TypedBuilder)]
pub struct MqttWill {
    #[builder(default = crate::packets::connect::ConnectWillProperties::new())]
    properties: crate::packets::connect::ConnectWillProperties,
    topic: MqttString,
    payload: MqttBytes,
    qos: mqtt_format::v5::qos::QualityOfService,
    retain: bool,
}

impl MqttWill {
    pub fn get_properties_mut(&mut self) -> &mut crate::packets::connect::ConnectWillProperties {
        &mut self.properties
    }
}

impl MqttWill {
    fn as_ref(&self) -> mqtt_format::v5::packets::connect::Will<'_> {
        mqtt_format::v5::packets::connect::Will {
            properties: self.properties.as_ref(),
            topic: self.topic.as_ref(),
            payload: self.payload.as_ref(),
            will_qos: self.qos,
            will_retain: self.retain,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MqttClientConnectError {
    #[error("An error occured while encoding or sending an MQTT Packet")]
    Send(#[source] MqttPacketCodecError),

    #[error("An error occured while decoding or receiving an MQTT Packet")]
    Receive(#[source] MqttPacketCodecError),

    #[error("The transport unexpectedly closed")]
    TransportUnexpectedlyClosed,

    #[error("The server sent a response with a protocol error: {reason}")]
    ServerProtocolError { reason: &'static str },
}

pub struct MqttClientConnector {
    transport: MqttConnectTransport,
    client_identifier: ProposedClientIdentifier,
    clean_start: CleanStart,
    keep_alive: KeepAlive,
    properties: crate::packets::connect::ConnectProperties,
    username: Option<MqttString>,
    password: Option<MqttBytes>,
    will: Option<MqttWill>,
}

impl MqttClientConnector {
    pub fn new(
        transport: MqttConnectTransport,
        client_identifier: ProposedClientIdentifier,
        clean_start: CleanStart,
        keep_alive: KeepAlive,
    ) -> MqttClientConnector {
        MqttClientConnector {
            transport,
            client_identifier,
            clean_start,
            keep_alive,
            properties: crate::packets::connect::ConnectProperties::new(),
            username: None,
            password: None,
            will: None,
        }
    }

    pub fn with_username(&mut self, username: MqttString) -> &mut Self {
        self.username = Some(username);
        self
    }

    pub fn with_password(&mut self, password: MqttBytes) -> &mut Self {
        self.password = Some(password);
        self
    }

    pub fn with_will(&mut self, will: MqttWill) -> &mut Self {
        self.will = Some(will);
        self
    }

    pub fn properties_mut(&mut self) -> &mut crate::packets::connect::ConnectProperties {
        &mut self.properties
    }
}

struct ConnectState {
    session_present: bool,
    receive_maximum: Option<NonZeroU16>,
    maximum_qos: Option<mqtt_format::v5::qos::MaximumQualityOfService>,
    retain_available: Option<bool>,
    topic_alias_maximum: Option<u16>,
    maximum_packet_size: Option<u32>,
    conn_write: FramedWrite<tokio::io::WriteHalf<MqttConnection>, MqttPacketCodec>,

    conn_read_recv: futures::channel::oneshot::Receiver<
        FramedRead<tokio::io::ReadHalf<MqttConnection>, MqttPacketCodec>,
    >,

    next_packet_identifier: std::num::NonZeroU16,
}

struct SessionState {
    client_identifier: MqttString,
    outstanding_packets: OutstandingPackets,
}

struct OutstandingPackets {
    packet_ident_order: Vec<std::num::NonZeroU16>,
    outstanding_packets:
        std::collections::BTreeMap<std::num::NonZeroU16, crate::packets::MqttPacket>,
}

impl OutstandingPackets {
    pub fn empty() -> Self {
        Self {
            packet_ident_order: Vec::new(),
            outstanding_packets: std::collections::BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, ident: std::num::NonZeroU16, packet: crate::packets::MqttPacket) {
        debug_assert_eq!(
            self.packet_ident_order.len(),
            self.outstanding_packets.len()
        );

        self.packet_ident_order.push(ident);
        let removed = self.outstanding_packets.insert(ident, packet);

        debug_assert!(removed.is_none());
    }

    pub fn update_by_id(
        &mut self,
        ident: std::num::NonZeroU16,
        packet: crate::packets::MqttPacket,
    ) {
        debug_assert_eq!(
            self.packet_ident_order.len(),
            self.outstanding_packets.len()
        );

        let removed = self.outstanding_packets.insert(ident, packet);

        debug_assert!(removed.is_some());
    }

    pub fn exists_outstanding_packet(&self, ident: std::num::NonZeroU16) -> bool {
        self.outstanding_packets.contains_key(&ident)
    }

    pub fn iter_in_send_order(
        &self,
    ) -> impl Iterator<Item = (std::num::NonZeroU16, &crate::packets::MqttPacket)> {
        self.packet_ident_order
            .iter()
            .flat_map(|id| self.outstanding_packets.get(id).map(|p| (*id, p)))
    }

    pub fn remove_by_id(&mut self, id: std::num::NonZeroU16) {
        // Vec::retain() preserves order
        self.packet_ident_order.retain(|&elm| elm != id);
        self.outstanding_packets.remove(&id);

        debug_assert_eq!(
            self.packet_ident_order.len(),
            self.outstanding_packets.len()
        );
    }
}

struct InnerClient {
    connection_state: Option<ConnectState>,
    session_state: Option<SessionState>,
}

pub struct MqttClient {
    inner: Arc<Mutex<InnerClient>>,
}

impl MqttClient {
    #[allow(clippy::new_without_default)]
    pub fn new() -> MqttClient {
        MqttClient {
            inner: Arc::new(Mutex::new(InnerClient {
                connection_state: None,
                session_state: None,
            })),
        }
    }

    pub async fn connect(
        &self,
        connector: MqttClientConnector,
    ) -> Result<Connected, MqttClientConnectError> {
        type Mcce = MqttClientConnectError;

        let inner_clone = self.inner.clone();
        let mut inner = self.inner.lock().await;
        let (read, write) = tokio::io::split(MqttConnection::from(connector.transport));
        let mut conn_write = FramedWrite::new(write, MqttPacketCodec);
        let mut conn_read = FramedRead::new(read, MqttPacketCodec);

        let conn_packet = mqtt_format::v5::packets::connect::MConnect {
            client_identifier: connector.client_identifier.as_str(),
            username: connector.username.as_ref().map(AsRef::as_ref),
            password: connector.password.as_ref().map(AsRef::as_ref),
            clean_start: connector.clean_start.as_bool(),
            will: connector.will.as_ref().map(|w| w.as_ref()),
            properties: connector.properties.as_ref(),
            keep_alive: connector.keep_alive.as_u16(),
        };

        conn_write
            .send(mqtt_format::v5::packets::MqttPacket::Connect(conn_packet))
            .await
            .map_err(Mcce::Send)?;

        let Some(maybe_connack) = conn_read.next().await else {
            return Err(Mcce::TransportUnexpectedlyClosed);
        };

        let maybe_connack = match maybe_connack {
            Ok(maybe_connack) => maybe_connack,
            Err(e) => {
                return Err(Mcce::Receive(e));
            }
        };

        let connack = loop {
            let can_use_auth = connector.properties.authentication_data.is_some();
            let _auth = match maybe_connack.get() {
                mqtt_format::v5::packets::MqttPacket::Connack(connack) => break connack,
                mqtt_format::v5::packets::MqttPacket::Auth(auth) => {
                    if can_use_auth {
                        auth
                    } else {
                        // MQTT-4.12.0-6
                        return Err(Mcce::ServerProtocolError {
                            reason: "MQTT-4.12.0-6",
                        });
                    }
                }
                _ => {
                    return Err(MqttClientConnectError::ServerProtocolError {
                        reason: "MQTT-3.1.4-5",
                    });
                }
            };

            // TODO: Use user-provided method to authenticate further

            todo!()
        };

        // TODO: Timeout here if the server doesn't respond

        if connack.reason_code == mqtt_format::v5::packets::connack::ConnackReasonCode::Success {
            // TODO: Read properties, configure client

            if connack.session_present && connector.clean_start == CleanStart::Yes {
                return Err(MqttClientConnectError::ServerProtocolError {
                    reason: "MQTT-3.2.2-2",
                });
            }

            let (conn_read_sender, conn_read_recv) = futures::channel::oneshot::channel();

            let connect_client_state = ConnectState {
                session_present: connack.session_present,
                receive_maximum: connack.properties.receive_maximum().map(|rm| rm.0),
                maximum_qos: connack.properties.maximum_qos().map(|mq| mq.0),
                retain_available: connack.properties.retain_available().map(|ra| ra.0),
                maximum_packet_size: connack.properties.maximum_packet_size().map(|mps| mps.0),
                topic_alias_maximum: connack.properties.topic_alias_maximum().map(|tam| tam.0),
                conn_write,
                conn_read_recv,
                next_packet_identifier: std::num::NonZeroU16::MIN,
            };

            let assigned_client_identifier = connack.properties.assigned_client_identifier();

            let client_identifier: MqttString;

            if let Some(aci) = assigned_client_identifier {
                if connector.client_identifier
                    == ProposedClientIdentifier::PotentiallyServerProvided
                {
                    client_identifier = MqttString::try_from(aci.0).map_err(|_mse| {
                        MqttClientConnectError::ServerProtocolError {
                            reason: "MQTT-1.5.4",
                        }
                    })?;
                } else {
                    return Err(MqttClientConnectError::ServerProtocolError {
                        reason: "MQTT-3.2.2.3.7",
                    });
                }
            } else {
                client_identifier = match connector.client_identifier {
                    ProposedClientIdentifier::PotentiallyServerProvided => {
                        return Err(MqttClientConnectError::ServerProtocolError {
                            reason: "MQTT-3.2.2.3.7",
                        });
                    }
                    ProposedClientIdentifier::MinimalRequired(mr) => mr.into_inner(),
                    ProposedClientIdentifier::PotentiallyAccepted(pa) => pa.into_inner(),
                };
            }

            inner.connection_state = Some(connect_client_state);
            inner.session_state = Some(SessionState {
                client_identifier,
                outstanding_packets: OutstandingPackets::empty(),
            });

            let connack_prop_view =
                crate::packets::connack::ConnackPropertiesView::try_from(maybe_connack)
                    .expect("An already matched value suddenly changed?");

            let background_task = async move {
                tracing::info!("Starting background task");
                let inner: Arc<Mutex<InnerClient>> = inner_clone;

                while let Some(next) = conn_read.next().await {
                    let process_span = tracing::debug_span!("Processing packet",
                                                            packet_kind = tracing::field::Empty,
                                                            packet_identifier = tracing::field::Empty);
                    tracing::debug!(parent: &process_span, valid = next.is_ok(), "Received packet");
                    let packet = match next {
                        Ok(packet) => packet,
                        Err(_) => todo!(),
                    };
                    process_span.record("packet_kind", tracing::field::debug(packet.get().get_kind()));

                    match packet.get() {
                        mqtt_format::v5::packets::MqttPacket::Auth(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Disconnect(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Pingreq(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Pingresp(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Puback(mpuback) => {
                            match mpuback.reason {
                                mqtt_format::v5::packets::puback::PubackReasonCode::Success |
                                mqtt_format::v5::packets::puback::PubackReasonCode::NoMatchingSubscribers => {
                                    // happy path
                                    let Some(ref mut session_state) = inner.lock().await.session_state else {
                                        tracing::error!(parent: &process_span, "No session state found");
                                        todo!()
                                    };

                                    let pident = std::num::NonZeroU16::try_from(mpuback.packet_identifier.0)
                                        .expect("Zero PacketIdentifier not valid here");
                                    process_span.record("packet_identifier", pident);

                                    if session_state.outstanding_packets.exists_outstanding_packet(pident) {
                                        session_state.outstanding_packets.remove_by_id(pident);
                                        tracing::trace!(parent: &process_span, "Removed packet id from outstanding packets");
                                    } else {
                                        tracing::error!(parent: &process_span, "Packet id does not exist in outstanding packets");
                                        todo!()
                                    }

                                    // TODO: Forward mpuback.properties etc to the user
                                }

                                mqtt_format::v5::packets::puback::PubackReasonCode::ImplementationSpecificError => todo!(),
                                mqtt_format::v5::packets::puback::PubackReasonCode::NotAuthorized => todo!(),
                                mqtt_format::v5::packets::puback::PubackReasonCode::PacketIdentifierInUse => todo!(),
                                mqtt_format::v5::packets::puback::PubackReasonCode::PayloadFormatInvalid => todo!(),
                                mqtt_format::v5::packets::puback::PubackReasonCode::QuotaExceeded => todo!(),
                                mqtt_format::v5::packets::puback::PubackReasonCode::TopicNameInvalid => todo!(),
                                mqtt_format::v5::packets::puback::PubackReasonCode::UnspecifiedError => todo!(),
                            }
                        },
                        mqtt_format::v5::packets::MqttPacket::Pubcomp(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Publish(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Pubrec(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Pubrel(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Suback(_) => todo!(),
                        mqtt_format::v5::packets::MqttPacket::Unsuback(_) => todo!(),

                        mqtt_format::v5::packets::MqttPacket::Connack(_) |
                        mqtt_format::v5::packets::MqttPacket::Connect(_) |
                        mqtt_format::v5::packets::MqttPacket::Subscribe(_) |
                        mqtt_format::v5::packets::MqttPacket::Unsubscribe(_) => {
                            todo!("Handle invalid packet")
                        }
                    }
                }

                tracing::debug!("Finished processing, returning reader");
                if let Err(conn_read) = conn_read_sender.send(conn_read) {
                    tracing::error!("Failed to return reader");
                    todo!()
                }

                Ok(())
            }
            .boxed::<>();

            return Ok(Connected {
                connack_prop_view,
                background_task,
            });
        }

        // TODO: Do something with error code

        todo!()
    }

    #[tracing::instrument(skip(self, payload), fields(payload_length = payload.as_ref().len()))]
    pub async fn publish(
        &self,
        topic: crate::topic::MqttTopic,
        qos: QualityOfService,
        retain: bool,
        payload: MqttPayload,
    ) -> Result<(), ()> {
        let mut inner = self.inner.lock().await;
        let inner = &mut *inner;

        let Some(conn_state) = &mut inner.connection_state else {
            tracing::error!("No connection state found");
            return Err(());
        };

        let Some(sess_state) = &mut inner.session_state else {
            tracing::error!("No session state found");
            return Err(());
        };

        if conn_state.retain_available.unwrap_or(true) && retain {
            tracing::warn!("Retain not available, but requested");
            return Err(());
        }

        let packet_identifier = if qos > QualityOfService::AtMostOnce {
            get_next_packet_ident(
                &mut conn_state.next_packet_identifier,
                &sess_state.outstanding_packets,
            )
            .map(Some)
            .map_err(|_| ())? // TODO
        } else {
            None
        };
        tracing::debug!(?packet_identifier, "Packet identifier computed");

        let publish = MPublish {
            duplicate: false,
            quality_of_service: qos.into(),
            retain,
            topic_name: topic.as_ref(),
            packet_identifier: packet_identifier
                .map(|nz| mqtt_format::v5::variable_header::PacketIdentifier(nz.get())),
            properties: mqtt_format::v5::packets::publish::PublishProperties::new(),
            payload: payload.as_ref(),
        };

        let packet = mqtt_format::v5::packets::MqttPacket::Publish(publish);

        let maximum_packet_size = conn_state
            .maximum_packet_size
            .unwrap_or(VARIABLE_INTEGER_MAX);

        if packet.binary_size() > maximum_packet_size {
            tracing::error!("Binary size bigger than maximum packet size");
            return Err(());
        }

        tracing::trace!(%maximum_packet_size, packet_size = packet.binary_size(), "Packet size");

        if let Some(pi) = packet_identifier {
            let mut bytes = tokio_util::bytes::BytesMut::new();
            bytes.reserve(packet.binary_size() as usize);
            let mut writer = crate::packets::MqttWriter(&mut bytes);
            packet.write(&mut writer).map_err(drop)?; // TODO
            let mqtt_packet = crate::packets::MqttPacket {
                packet: yoke::Yoke::try_attach_to_cart(
                    crate::packets::StableBytes(bytes.freeze()),
                    |bytes: &[u8]| mqtt_format::v5::packets::MqttPacket::parse_complete(bytes),
                )
                .unwrap(), // TODO
            };

            sess_state.outstanding_packets.insert(pi, mqtt_packet);
        }

        tracing::trace!("Publishing");
        conn_state
            .conn_write
            .send(packet)
            .in_current_span()
            .await
            .unwrap();
        tracing::trace!("Finished publishing");

        Ok(())
    }
}

fn get_next_packet_ident(
    next_packet_ident: &mut std::num::NonZeroU16,
    outstanding_packets: &OutstandingPackets,
) -> Result<std::num::NonZeroU16, PacketIdentifierExhausted> {
    let start = *next_packet_ident;

    loop {
        let next = *next_packet_ident;

        if !outstanding_packets.exists_outstanding_packet(next) {
            return Ok(next);
        }

        match next_packet_ident.checked_add(1) {
            Some(n) => *next_packet_ident = n,
            None => *next_packet_ident = std::num::NonZeroU16::MIN,
        }

        if start == *next_packet_ident {
            return Err(PacketIdentifierExhausted);
        }
    }
}

#[must_use]
pub struct Connected {
    pub connack_prop_view: ConnackPropertiesView,
    pub background_task: futures::future::BoxFuture<'static, Result<(), ()>>,
}

#[derive(Debug, thiserror::Error)]
#[error("No free packet identifiers available")]
pub struct PacketIdentifierExhausted;

#[cfg(test)]
mod tests {
    use crate::client::MqttClient;

    static_assertions::assert_impl_all!(MqttClient: Send, Sync);
}
