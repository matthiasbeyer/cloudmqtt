//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

pub mod builder;
pub mod connect;
mod pattern;
mod receive;
pub mod send;
mod state;
mod subscriptions;

use std::sync::Arc;

use futures::lock::Mutex;
use pattern::Pattern;
use subscriptions::Subscription;
use subscriptions::Subscriptions;

use self::send::Callbacks;
use self::send::ClientHandlers;
use self::state::ConnectState;
use self::state::SessionState;

struct InnerClient {
    connection_state: Option<ConnectState>,
    session_state: Option<SessionState>,
    default_handlers: ClientHandlers,
    outstanding_callbacks: Callbacks,
    subscriptions: Subscriptions,
}

pub struct MqttClient {
    inner: Arc<Mutex<InnerClient>>,
}

impl MqttClient {
    pub fn new_with_default_handlers() -> MqttClient {
        MqttClient {
            inner: Arc::new(Mutex::new(InnerClient {
                connection_state: None,
                session_state: None,
                default_handlers: ClientHandlers::default(),
                outstanding_callbacks: Callbacks::new(),
                subscriptions: Subscriptions::new(),
            })),
        }
    }

    pub fn builder() -> builder::MqttClientBuilder {
        builder::MqttClientBuilder::new()
    }

    pub async fn subscribe(&self, pattern: Pattern) -> Subscription {
        self.inner
            .lock()
            .await
            .subscriptions
            .create_subscription(pattern)
            .await
    }
}

#[cfg(test)]
mod tests {
    use crate::client::ClientHandlers;
    use crate::client::MqttClient;

    static_assertions::assert_impl_all!(MqttClient: Send, Sync);
    static_assertions::assert_impl_all!(ClientHandlers: Send);
}
