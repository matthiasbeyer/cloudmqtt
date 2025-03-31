//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

pub struct TestHarness {}

impl TestHarness {
    pub fn new() -> Self {
        Self {}
    }

    pub fn start_broker(&mut self, _name: String) -> Result<(), TestHarnessError> {
        Ok(())
    }

    pub fn create_client(&mut self, _name: String) -> Result<(), TestHarnessError> {
        Ok(())
    }

    pub fn connect_client_to_broker(
        &mut self,
        _client_name: String,
        _broker_name: String,
    ) -> Result<(), TestHarnessError> {
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TestHarnessError {}
