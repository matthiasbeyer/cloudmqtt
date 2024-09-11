use std::str::FromStr;

use crate::topic::MqttTopic;

#[derive(Debug, Eq, PartialEq, Hash)]
pub struct Pattern(String);

#[derive(Debug, thiserror::Error)]
pub enum MqttPatternError {}

impl FromStr for Pattern {
    type Err = MqttPatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Pattern(s.to_string()))
    }
}

impl Pattern {
    pub fn matches(&self, other: &MqttTopic) -> bool {
        let mut self_fragments = self.fragments().peekable();
        let Some(mut self_current_fragment) = self_fragments.next() else {
            return false;
        };

        let mut other_fragments = other.fragments().peekable();

        let Some(mut other_current_fragment) = other_fragments.next() else {
            return false;
        };

        loop {
            tracing::trace!(?self_current_fragment, ?other_current_fragment);
            // Single wildcard
            if self_current_fragment == "+" {
                tracing::trace!("Found single wildcard");
                self_current_fragment = match self_fragments.next() {
                    Some(f) => f,
                    None => return true,
                };

                other_current_fragment = match other_fragments.next() {
                    Some(f) => f,
                    None => return true,
                };

                continue;
            }

            // Multi wildcard
            if self_current_fragment == "#" {
                tracing::trace!("Found multi wildcard");

                while {
                    let other_peek = other_fragments.peek();
                    let self_peek = self_fragments.peek();

                    other_peek.is_some() && other_peek != self_peek
                } {
                    tracing::trace!(?other_current_fragment, "Fetching next other fragment");
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
                    None => return true,
                };
            } else {
                return false;
            }
        }
    }

    fn fragments(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::Pattern;
    use crate::topic::MqttTopic;

    fn p(s: &str) -> Pattern {
        Pattern::from_str(s).unwrap()
    }

    fn t(s: &str) -> MqttTopic {
        MqttTopic::try_from(s).unwrap()
    }

    #[test_log::test]
    fn test_matches() {
        assert!(p("a").matches(&t("a")));
        assert!(p("a").matches(&t("a")));
        assert!(p("+").matches(&t("a")));
        assert!(p("#").matches(&t("a")));
    }

    #[test_log::test]
    fn test_matches_not() {
        assert!(!p("b").matches(&t("a")));
        assert!(!p("a").matches(&t("b")));
    }
}
