//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::collections::HashMap;
use std::collections::VecDeque;

use futures::FutureExt;
use futures::Stream;

use crate::packets::publish::Publish;

type Receiver = tokio::sync::mpsc::Receiver<Publish>;
type Sender = tokio::sync::mpsc::Sender<Publish>;

use super::pattern::Pattern;
use super::pattern::PatternFragment;

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
    subscriptions: SubscriptionTopic,
}

#[derive(Debug, Clone, Default)]
struct SubscriptionTopic {
    subscriptions: Vec<Sender>,
    children: HashMap<PatternFragment, SubscriptionTopic>,
}

impl SubscriptionTopic {
    fn add_subscription(&mut self, mut pattern: VecDeque<PatternFragment>, sender: Sender) {
        match pattern.pop_front() {
            None => self.subscriptions.push(sender),
            Some(filter) => self
                .children
                .entry(filter)
                .or_default()
                .add_subscription(pattern, sender),
        }
    }
}

impl Subscriptions {
    pub(crate) fn new() -> Self {
        Self {
            subscriptions: SubscriptionTopic::default(),
        }
    }

    pub(crate) async fn create_subscription(&mut self, pattern: Pattern) -> Subscription {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        self.subscriptions
            .add_subscription(pattern.into_inner(), sender);
        Subscription::new(receiver)
    }

    pub(crate) async fn handle_publish<'s>(&'s self, packet: Publish) -> Result<(), ()> {
        let topic_name = TopicName::parse_from(packet.get().topic_name);
        let mut i = topic_name
            .get_matches(0, &self.subscriptions)
            .zip(std::iter::repeat(packet));

        while let Some((sender, packet)) = i.next() {
            if let Err(error) = sender.send(packet).await {
                tracing::warn!(?error, "Failed to send publish to subscription");
                return Err(()); // TODO
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct TopicName(VecDeque<String>);

impl TopicName {
    fn parse_from(topic: &str) -> TopicName {
        TopicName(topic.split('/').map(String::from).collect())
    }

    fn get_matches<'a>(
        &'a self,
        idx: usize,
        routing: &'a SubscriptionTopic,
    ) -> Box<dyn Iterator<Item = Sender> + 'a> {
        let multi_wild = routing
            .children
            .get(&PatternFragment::MultiWildcard)
            .into_iter()
            .flat_map(|child| child.subscriptions.iter().map(Sender::clone))
            .inspect(|sub| tracing::trace!(?sub, "Matching MultiWildcard topic"));

        let single_wild = routing
            .children
            .get(&PatternFragment::SingleWildcard)
            .into_iter()
            .flat_map(move |child| self.get_matches(idx + 1, child))
            .inspect(|sub| tracing::trace!(?sub, "Matching SingleWildcard topic"));

        let nested_named = self
            .0
            .get(idx)
            .and_then(|topic_level| {
                routing
                    .children
                    .get(&PatternFragment::Named(topic_level.to_string()))
            })
            .map(move |child| self.get_matches(idx + 1, child));

        let current_named = if idx == self.0.len() {
            Some(routing.subscriptions.iter().map(Sender::clone))
        } else {
            None
        };

        Box::new(
            multi_wild
                .chain(single_wild)
                .chain(nested_named.into_iter().flatten())
                .chain(current_named.into_iter().flatten()),
        )
    }
}
