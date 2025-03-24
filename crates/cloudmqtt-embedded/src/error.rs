//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use crate::stack_resources::MqttStackResourceWriteSlotError;

#[derive(Debug)]
pub enum CloudmqttClientError {
    Connect(embassy_net::tcp::ConnectError),
    WriteBuf(MqttStackResourceWriteSlotError),
    ConnectionReset,
}
