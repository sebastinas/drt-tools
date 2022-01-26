// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs::File,
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use anyhow::Result;
use indicatif::ProgressStyle;
use xdg::BaseDirectories;

const PROGRESS_CHARS: &str = "█  ";

pub(crate) fn default_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar().progress_chars(PROGRESS_CHARS)
}

pub(crate) struct Cache {
    base_directory: BaseDirectories,
}

impl Cache {
    pub fn new() -> Result<Self> {
        Ok(Self {
            base_directory: BaseDirectories::with_prefix("Debian-RT-tools")?,
        })
    }

    pub fn get_cache_path<P>(&self, path: P) -> Result<PathBuf>
    where
        P: AsRef<Path>,
    {
        Ok(self.base_directory.place_cache_file(path)?)
    }

    pub fn get_cache_bufreader<P>(&self, path: P) -> Result<BufReader<File>>
    where
        P: AsRef<Path>,
    {
        Ok(BufReader::new(File::open(self.get_cache_path(path)?)?))
    }

    pub fn get_data_bufreader<P>(&self, path: P) -> Result<BufReader<File>>
    where
        P: AsRef<Path>,
    {
        Ok(BufReader::new(File::open(
            self.base_directory.place_data_file(path)?,
        )?))
    }

    pub fn get_data_bufwriter<P>(&self, path: P) -> Result<BufWriter<File>>
    where
        P: AsRef<Path>,
    {
        Ok(BufWriter::new(File::create(
            self.base_directory.place_data_file(path)?,
        )?))
    }
}
