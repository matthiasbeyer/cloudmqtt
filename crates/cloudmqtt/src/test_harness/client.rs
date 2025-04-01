//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::sync::Arc;

use super::broker::Broker;
use super::error::TestHarnessError;
use crate::codec::MqttPacket;

pub(crate) struct Client {
    client: Arc<crate::CloudmqttClient>,
    sender: Option<mpsc::UnboundedSender<Result<MqttPacket, crate::codec::MqttPacketCodecError>>>,
    name: String,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Client {
    pub(crate) fn new(name: String) -> Self {
        Self {
            client: Arc::new(crate::CloudmqttClient::new()),
            sender: None,
            name,
        }
    }

    pub(crate) async fn connect_to(&mut self, broker: &mut Broker) -> Result<(), TestHarnessError> {
        let (client, server) = tokio::io::duplex(100);

        self.client.connect(client).await.unwrap();
        broker.connect(self.name.clone(), server).await.unwrap();
        Ok(())
    }
}
