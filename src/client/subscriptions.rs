//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::sync::Arc;

use crate::packets::publish::Publish;

type Receiver = tokio::sync::mpsc::Receiver<Publish>;
type Sender = tokio::sync::mpsc::Sender<Publish>;

use crate::topic::MqttTopic;

#[derive(Debug)]
pub(crate) struct Subscription {
    topic: MqttTopic,
    recv: Receiver,
}

impl Subscription {
    fn new(topic: MqttTopic, recv: Receiver) -> Self {
        Self { topic, recv }
    }

    fn matches(&self, topic: &str) -> bool {
        todo!()
    }
}

/// Efficient storage for MQTT Topic -> Subscription mapping
pub(crate) struct Subscriptions {
    // naive impl
    subscriptions: Vec<(Arc<Subscription>, Sender)>,
}

impl Subscriptions {
    pub(crate) fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    pub(crate) async fn create_subscription(&mut self, topic: MqttTopic) -> Arc<Subscription> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let sub = Arc::new(Subscription::new(topic, receiver));
        self.subscriptions.push((sub.clone(), sender));
        sub
    }

    pub(crate) async fn handle_publish(&self, packet: Publish) -> Result<Option<()>, ()> {
        let Some((_, sender)) = self
            .subscriptions
            .iter()
            .find(|(subscr, _sender)| subscr.matches(&packet.get().topic_name))
        else {
            return Ok(None);
        };

        if let Err(error) = sender.send(packet).await {
            tracing::warn!(?error, "Failed to send publish to subscription");
            Err(())
        } else {
            Ok(Some(()))
        }
    }
}
