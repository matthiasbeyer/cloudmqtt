//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

pub struct MqttStackResources<const RECV_BUF_SIZE: usize, const SEND_BUF_SIZE: usize> {
    recv_buf: [(); RECV_BUF_SIZE],
    send_buf: [MqttStackResourceWriteSlot; SEND_BUF_SIZE],
}

impl<const RECV_BUF_SIZE: usize, const SEND_BUF_SIZE: usize>
    MqttStackResources<RECV_BUF_SIZE, SEND_BUF_SIZE>
{
    // TODO: Error handling
    pub(crate) fn get_next_send_buf_mut(&mut self) -> &mut MqttStackResourceWriteSlot {
        self.send_buf
            .iter_mut()
            .find(|s| s.next_write_idx == 0)
            .unwrap()
    }
}

pub(crate) struct MqttStackResourceWriteSlot {
    next_write_idx: usize,
    buf: [u8; 1024], // TODO
}

impl MqttStackResourceWriteSlot {
    pub(crate) fn new() -> Self {
        Self {
            next_write_idx: 0,
            buf: [0; 1024],
        }
    }

    pub(crate) fn clear(&mut self) {
        self.next_write_idx = 0;
    }

    pub(crate) fn as_slice(&self) -> &[u8] {
        &self.buf[0..self.next_write_idx]
    }
}

#[derive(Debug)]
pub enum MqttStackResourceWriteSlotError {
    Invariant,
}

impl From<mqtt_format::v5::write::MqttWriteError> for MqttStackResourceWriteSlotError {
    fn from(_value: mqtt_format::v5::write::MqttWriteError) -> Self {
        Self::Invariant
    }
}

impl mqtt_format::v5::write::WriteMqttPacket for MqttStackResourceWriteSlot {
    type Error = MqttStackResourceWriteSlotError;

    fn write_byte(&mut self, u: u8) -> mqtt_format::v5::write::WResult<Self> {
        self.buf[self.next_write_idx] = u;
        self.next_write_idx += 1;
        Ok(())
    }

    fn write_slice(&mut self, u: &[u8]) -> mqtt_format::v5::write::WResult<Self> {
        for i in u {
            self.write_byte(*i)?;
        }
        Ok(())
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.buf.len()
    }
}
