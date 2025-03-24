//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#![no_std]

use cloudmqtt_core::client::ExpectedAction;
use cloudmqtt_core::client::MqttClientFSM;
use cloudmqtt_core::client::MqttInstant;
use embassy_time::Instant;
use mqtt_format::v5::qos::QualityOfService;
use mqtt_format::v5::write::MqttWriteError;

pub mod macros;
pub mod stack_resources;
pub mod error;
pub mod token;

use crate::error::CloudmqttClientError;
use crate::stack_resources::MqttStackResources;
use crate::token::ConnectedToken;

pub struct Subscription {
    pub topic: &'static str,
    pub qos: QualityOfService,
    pub retain: bool,
}

impl Subscription {
    pub const fn new(topic: &'static str, qos: QualityOfService, retain: bool) -> Self {
        Self { topic, qos, retain }
    }
}

pub struct CloudmqttClient<
    'c,
    const SUBSCRIPTIONS_LEN: usize,
    const RECV_BUF_SIZE: usize,
    const SEND_BUF_SIZE: usize,
> {
    broker_addr: embassy_net::IpAddress,
    broker_port: u16,
    fsm: MqttClientFSM,
    stack_resources: &'c mut MqttStackResources<RECV_BUF_SIZE, SEND_BUF_SIZE>,
    subscriptions: [Subscription; SUBSCRIPTIONS_LEN],
    socket: embassy_net::tcp::TcpSocket<'c>,

    last_operation: Option<MqttInstant>,
}

fn since(start: MqttInstant) -> MqttInstant {
    MqttInstant::const_new(start.elapsed_seconds(MqttInstant::const_new(Instant::now().as_secs())))
}

impl<'c, const SUBSCRIPTIONS_LEN: usize, const RECV_BUF_SIZE: usize, const SEND_BUF_SIZE: usize>
    CloudmqttClient<'c, SUBSCRIPTIONS_LEN, RECV_BUF_SIZE, SEND_BUF_SIZE>
{
    pub const fn new(
        broker_addr: embassy_net::IpAddress,
        broker_port: u16,
        subscriptions: [Subscription; SUBSCRIPTIONS_LEN],
        stack_resources: &'c mut MqttStackResources<RECV_BUF_SIZE, SEND_BUF_SIZE>,
        socket: embassy_net::tcp::TcpSocket<'c>,
    ) -> Result<Self, CloudmqttClientError> {
        Ok(Self {
            broker_addr,
            broker_port,
            fsm: MqttClientFSM::const_new(
                cloudmqtt_core::client::UsizePacketIdentifierStore::const_new(),
            ),
            subscriptions,
            stack_resources,
            socket,
            last_operation: None,
        })
    }

    pub async fn connect(&mut self) -> Result<ConnectedToken, CloudmqttClientError> {
        if let Err(error) = self
            .socket
            .connect((self.broker_addr, self.broker_port))
            .await
        {
            defmt::error!(
                "Failed to connect to MQTT Broker ({}:{}): {:?}",
                self.broker_addr,
                self.broker_port,
                error
            );
            return Err(CloudmqttClientError::Connect(error));
        }
        defmt::info!("TCP Socket connected");

        let last_operation = self
            .last_operation
            .clone()
            .unwrap_or(MqttInstant::const_new(0));

        match self.fsm.handle_connect(
            since(last_operation),
            mqtt_format::v5::packets::connect::MConnect {
                client_identifier: "cloudmqtt-0",
                username: None,
                password: None,
                clean_start: true,
                will: None,
                properties: mqtt_format::v5::packets::connect::ConnectProperties::new(),
                keep_alive: 0,
            },
        ) {
            ExpectedAction::SendPacket(packet) => self.send_packet(packet).await,
            _other => unreachable!(),
        }
    }

    async fn send_packet(
        &mut self,
        packet: mqtt_format::v5::packets::MqttPacket<'_>,
    ) -> Result<ConnectedToken, CloudmqttClientError> {
        let buf = self.stack_resources.get_next_send_buf_mut();
        if let Err(error) = packet.write(buf) {
            defmt::error!(
                "Failed to write MQTT packet to buffer: {}",
                defmt::Debug2Format(&error)
            );
            buf.clear();
            return Err(CloudmqttClientError::WriteBuf(error));
        }
        if let Err(embassy_net::tcp::Error::ConnectionReset) =
            self.socket.write(buf.as_slice()).await
        {
            defmt::error!("Failed to write MQTT packet to socket: Connection reset",);
            buf.clear();
            return Err(CloudmqttClientError::ConnectionReset);
        }

        buf.clear();
        self.last_operation = Some(MqttInstant::const_new(Instant::now().as_secs()));
        Ok(ConnectedToken(()))
    }

    pub async fn publish_qos0(
        &mut self,
        token: ConnectedToken,
        topic: &str,
        payload: &[u8],
        retain: bool,
    ) -> PublishResult {
        let mut publisher = self
            .fsm
            .publish(mqtt_format::v5::packets::publish::MPublish {
                duplicate: false,
                quality_of_service: mqtt_format::v5::qos::QualityOfService::AtMostOnce,
                retain,
                topic_name: topic,
                packet_identifier: None,
                properties: mqtt_format::v5::packets::publish::PublishProperties::new(),
                payload,
            });

        let action = publisher.run(MqttInstant::const_new(
            embassy_time::Instant::now().as_secs(),
        ));
        match action {
            Some(ExpectedAction::SendPacket(packet)) => {
                match self.send_packet(packet).await {
                    Ok(connected) => PublishResult::Ok { connected },
                    Err(CloudmqttClientError::ConnectionReset) => PublishResult::Disconnected,
                    Err(error) => {
                        // TODO: Do we need to roll back the FSM???
                        PublishResult::Err(error)
                    }
                }
            }
            Some(ExpectedAction::StorePacket { .. }) => {
                // unexpected because of QOS0
                todo!()
            }

            Some(ExpectedAction::ReleasePacket { .. }) => {
                // unexpected because of QOS0
                todo!()
            }

            Some(_other) => {
                todo!()
            }

            None => {
                // unexpected because we want to send!
                todo!()
            }
        }
    }
}

#[derive(Debug)]
pub enum PublishResult {
    /// Everything fine
    Ok { connected: ConnectedToken },

    /// We are disconnected, nothing has been sent
    Disconnected,

    /// We errored
    Err(CloudmqttClientError),
}
