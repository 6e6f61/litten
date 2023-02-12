use serde::{Deserializer, Deserialize};
use std::borrow::Cow;
use serde::de::Error;

#[derive(Debug, Clone)]
pub struct Regex(pub regex::Regex);

impl std::default::Default for Regex {
    fn default() -> Self {
        Regex(regex::Regex::new(".*").unwrap())
    }
}

impl<'de> Deserialize<'de> for Regex {
    fn deserialize<D>(d: D) -> Result<Regex, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = <Cow<str>>::deserialize(d)?;

        match s.parse() {
            Ok(regex) => Ok(Regex(regex)),
            Err(err) => Err(D::Error::custom(err)),
        }
    }
}