//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#[macro_export]
macro_rules! subscription {
    {
        topic: $topic:literal,
        qos: $qos:expr,
        retain: $retain:literal,
    } => {
        Subscription {
            topic: $topic,
            qos: $qos,
            retain: $retain,
        }
    }

}
pub use subscription;
