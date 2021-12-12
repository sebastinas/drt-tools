// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use indicatif::ProgressStyle;

const PROGRESS_CHARS: &str = "█  ";

pub(crate) fn default_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar().progress_chars(PROGRESS_CHARS)
}
