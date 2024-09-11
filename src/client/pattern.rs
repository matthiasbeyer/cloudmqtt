use std::collections::VecDeque;
use std::str::FromStr;

pub struct Pattern(VecDeque<PatternFragment>);

impl FromStr for Pattern {
    type Err = MqttPatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split('/')
            .map(PatternFragment::from_str)
            .collect::<Result<VecDeque<_>, MqttPatternError>>()
            .map(Pattern)
    }
}

impl Pattern {
    pub(crate) fn into_inner(self) -> VecDeque<PatternFragment> {
        self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MqttPatternError {}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PatternFragment {
    MultiWildcard,
    SingleWildcard,
    Named(String),
}

impl FromStr for PatternFragment {
    type Err = MqttPatternError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "#" => PatternFragment::MultiWildcard,
            "+" => PatternFragment::SingleWildcard,
            name => PatternFragment::Named(name.to_owned()),
        })
    }
}
