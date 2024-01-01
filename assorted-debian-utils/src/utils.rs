// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

//! # Utils used by other modules.

use std::{
    any::type_name,
    fmt::{Display, Formatter},
    marker::PhantomData,
};

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

/// Helper to parse anything that implements `TryFrom<&str>``
#[derive(Debug)]
pub(crate) struct TryFromStrVisitor<T>(PhantomData<T>);

impl<T> TryFromStrVisitor<T> {
    pub(crate) fn new() -> Self {
        Self(PhantomData)
    }
}

impl<'de, T> serde::de::Visitor<'de> for TryFromStrVisitor<T>
where
    for<'a> T: TryFrom<&'a str>,
    for<'a> <T as TryFrom<&'a str>>::Error: Display,
{
    type Value = T;

    fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
        write!(formatter, "a {}", type_name::<T>())
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        T::try_from(s).map_err(E::custom)
    }
}
