// Copyright 2021-2022 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Utils used by other modules.

use std::fmt::Formatter;

use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use serde::de;

/// Helper to parse date-time strings as UTC based on a given format
#[derive(Debug)]
pub(crate) struct DateTimeVisitor(pub &'static str);

impl<'de> de::Visitor<'de> for DateTimeVisitor {
    type Value = DateTime<Utc>;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "a date and time formatted as '{}'", self.0)
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        NaiveDateTime::parse_from_str(s, self.0)
            .map_err(|_| de::Error::invalid_value(de::Unexpected::Str(s), &self))
            .map(|naive_date_time| Utc.from_utc_datetime(&naive_date_time))
    }
}
