//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

#[derive(Debug)]
pub struct ConnectedToken(pub(crate) ());

static_assertions::assert_not_impl_all!(ConnectedToken: Clone, Copy);
static_assertions::assert_eq_size!(ConnectedToken, ());
