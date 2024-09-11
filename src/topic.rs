//
//   This Source Code Form is subject to the terms of the Mozilla Public
//   License, v. 2.0. If a copy of the MPL was not distributed with this
//   file, You can obtain one at http://mozilla.org/MPL/2.0/.
//

use std::str::FromStr;

use crate::string::MqttString;
use crate::string::MqttStringError;

#[derive(Debug)]
pub struct MqttTopic(MqttString);

impl AsRef<str> for MqttTopic {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MqttTopicError {
    #[error(transparent)]
    String(#[from] MqttStringError),

    #[error("MQTT Topics are not allowed to be empty")]
    Empty,

    #[error("MQTT Topics are not allowed to contain a NULL (U+0000) character")]
    Null,

    #[error("MQTT Topics are not allowed to contain MQTT wildcard characters ('#' or '+')")]
    Wildcard,
}

impl FromStr for MqttTopic {
    type Err = MqttTopicError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(MqttTopicError::Empty);
        }

        if s.contains('\0') {
            return Err(MqttTopicError::Null);
        }

        if s.contains(['#', '+']) {
            return Err(MqttTopicError::Wildcard);
        }

        // MQTTString checks the length for us
        Ok(MqttTopic(MqttString::from_str(s)?))
    }
}

impl TryFrom<String> for MqttTopic {
    type Error = MqttTopicError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl TryFrom<&str> for MqttTopic {
    type Error = MqttTopicError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

impl MqttTopic {
    pub fn matches(&self, other: &MqttTopic) -> bool {
        let mut result = false;

        let mut self_fragments = self.fragments().peekable();
        let Some(mut self_current_fragment) = self_fragments.next() else {
            return result;
        };

        let mut other_fragments = other.fragments().peekable();

        let Some(mut other_current_fragment) = other_fragments.next() else {
            return result;
        };

        loop {
            // Single wildcard
            if self_current_fragment == "+" {
                self_current_fragment = match self_fragments.next() {
                    Some(f) => f,
                    None => return result
                };

                other_current_fragment = match other_fragments.next() {
                    Some(f) => f,
                    None => return result
                };

                continue
            }

            // Multi wildcard
            if self_current_fragment == "#" {
                while other_fragments.peek() != self_fragments.peek() {
                    other_current_fragment = match other_fragments.next() {
                        None => return true,
                        Some(o) => o,
                    };
                }

                continue;
            }

            if self_current_fragment == other_current_fragment {
                self_current_fragment = match self_fragments.next() {
                    Some(n) => n,
                    None => return true,
                };

                other_current_fragment = match other_fragments.next() {
                    Some(n) => n,
                    None => return result,
                };
            } else {
                return result
            }
        }
    }

    fn fragments(&self) -> impl Iterator<Item = &str> {
        self.as_ref().split('/')
    }
}

#[cfg(test)]
mod tests {
    use super::MqttTopic;

    fn t(s: &str) -> MqttTopic {
        MqttTopic::try_from(s).unwrap()
    }

    #[test]
    fn test_matches() {
        assert!(t("a").matches(&t("a")));
        assert!(t("a").matches(&t("a")));
        assert!(t("+").matches(&t("a")));
        assert!(t("#").matches(&t("a")));
    }

    #[test]
    fn test_matches_not() {
        assert!(!t("b").matches(&t("a")));
        assert!(!t("a").matches(&t("b")));
    }
}
