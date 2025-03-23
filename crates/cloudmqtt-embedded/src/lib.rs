//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#![no_std]

use cloudmqtt_core::client::{ExpectedAction, MqttClientFSM};
use embassy_time::Instant;
use mqtt_format::v5::qos::QualityOfService;

pub mod macros;

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

pub struct CloudmqttClient<'c, const SUBSCRIPTIONS_LEN: usize> {
    broker_addr: &'c str,
    broker_port: u16,
    fsm: MqttClientFSM,
    subscriptions: [Subscription; SUBSCRIPTIONS_LEN],
}

fn since(start: Instant) -> MqttInstant {
    MqttInstant::new(start.elapsed().as_secs())
}

impl<'c, const SUBSCRIPTIONS_LEN: usize> CloudmqttClient<'c, SUBSCRIPTIONS_LEN> {
    pub const fn start(
        broker_addr: &'c str,
        broker_port: u16,
        subscriptions: [Subscription; SUBSCRIPTIONS_LEN],
    ) -> Result<Self, CloudmqttClientError> {
        Ok(Self {
            broker_addr,
            broker_port,
            fsm: MqttClientFSM::default(),
            subscriptions,
        })
    }

    pub fn get_next_action(&mut self) -> Result<Action, CloudmqttClientError> {
        if !self.connected {
            return self.connect()
        }
        todo!()
    }

    fn connect(&mut self) -> Result<Action, CloudmqttClientError> {
        self.fsm.handle_connect(
            since(start),
            mqtt_format::v5::packets::connect::MConnect {
                client_identifier: "cloudmqtt-0",
                username: None,
                password: None,
                clean_start: true,
                will: None,
                properties: mqtt_format::v5::packets::connect::ConnectProperties::new(),
                keep_alive: 0,
            },
        ).map(|action| match action {
            ExpectedAction::SendPacket(packet) => Action::Send(packet),
            other => unreachable!(),
        })
    }
}

pub enum Action {
}

#[derive(Debug)]
pub enum CloudmqttClientError {}
