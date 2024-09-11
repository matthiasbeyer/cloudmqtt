//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use crate::packets::publish::Publish;

type Receiver = tokio::sync::mpsc::Receiver<Publish>;
type Sender = tokio::sync::mpsc::Sender<Publish>;

use crate::topic::MqttTopic;

use super::pattern::Pattern;

#[derive(Debug)]
pub(crate) struct Subscription {
    recv: Receiver,
}

impl Subscription {
    fn new(recv: Receiver) -> Self {
        Self { recv }
    }

    pub async fn next(&mut self) -> Option<Publish> {
        self.recv.recv().await
    }
}

/// Efficient storage for MQTT Topic -> Subscription mapping
///
/// # TODO
///
/// Currently this is not implemented in a efficient way
pub(crate) struct Subscriptions {
    // naive impl
    subscriptions: Vec<(Pattern, Sender)>,
}

impl Subscriptions {
    pub(crate) fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    pub(crate) async fn create_subscription(&mut self, pattern: Pattern) -> Subscription {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscriptions.push((pattern, sender));
        Subscription::new(receiver)
    }

    pub(crate) async fn handle_publish(&self, packet: Publish) -> Result<Option<()>, ()> {
        let topic = MqttTopic::try_from(packet.get().topic_name).unwrap(); // TODO
        // naive search
        let Some((_, sender)) = self
            .subscriptions
            .iter()
            .find(|(pattern, _sender)| pattern.matches(&topic))
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

fn topic_matches(topic_a: &MqttTopic, topic_b: &str) -> bool {
    todo!()
}
