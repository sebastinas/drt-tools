// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Utils used by other modules.

use chrono::{DateTime, TimeZone, Utc};
use serde::de;
use std::fmt;

#[derive(Debug)]
pub(crate) struct DateTimeVisitor<'a>(pub &'a str);

impl<'de> de::Visitor<'de> for DateTimeVisitor<'_> {
    type Value = DateTime<Utc>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a date and time formatted as {}", self.0)
    }

    fn visit_str<E>(self, s: &str) -> std::result::Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Utc.datetime_from_str(s, self.0) {
            Ok(dt) => Ok(dt),
            Err(_) => Err(de::Error::invalid_value(de::Unexpected::Str(s), &self)),
        }
    }
}
