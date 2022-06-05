// Copyright 2021 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use assorted_debian_utils::{
    architectures::{Architecture, RELEASE_ARCHITECTURES},
    archive::{Codename, Suite},
};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use log::debug;
use reqwest::{header, Client, Response, StatusCode};
use xdg::BaseDirectories;
use xz2::write::XzDecoder;

const PROGRESS_CHARS: &str = "█  ";

pub(crate) fn default_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar().progress_chars(PROGRESS_CHARS)
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CacheEntries {
    Excuses,
    Packages,
    FTBFSBugs(Codename),
    AutoRemovals,
    OutdatedBuiltUsing,
    // Sources,
    Contents(Suite),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CacheState {
    NoUpdate,
    FreshFiles,
}

#[derive(Debug)]
struct Downloader {
    always_download: bool,
    client: Client,
}

impl Downloader {
    pub fn new(always_download: bool) -> Self {
        Self {
            always_download,
            client: Client::new(),
        }
    }

    async fn download_init<P>(&self, url: &str, path: P) -> Result<Option<(Response, ProgressBar)>>
    where
        P: AsRef<Path>,
    {
        debug!("Starting download of {} to {:?}", url, path.as_ref());
        let res = self.client.get(url);
        let res = if !self.always_download {
            if let Ok(dst_metadata) = fs::metadata(path) {
                // if always_download was not set and we have local copy, tell the server the date
                res.header(
                    header::IF_MODIFIED_SINCE,
                    httpdate::fmt_http_date(dst_metadata.modified()?),
                )
            } else {
                res
            }
        } else {
            res
        }
        .send()
        .await
        .with_context(|| format!("Failed to GET from '{}'", &url))?;

        if !self.always_download && res.status() == StatusCode::NOT_MODIFIED {
            // this will only trigger if always_download is not set and the server reports that the
            // file was not modified
            debug!(
                "Skipping {}: always_download is not set and the file was not modified",
                url
            );
            return Ok(None);
        }

        if let Some(total_size) = res.content_length() {
            let pb = ProgressBar::new(total_size);
            pb.set_style(default_progress_style()
            .template("{msg}: {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            );
            pb.set_message(format!("Downloading {}", url));
            Ok(Some((res, pb)))
        } else {
            Ok(Some((res, ProgressBar::hidden())))
        }
    }

    async fn download_internal(
        &self,
        res: Response,
        pb: &ProgressBar,
        writer: &mut impl Write,
    ) -> Result<()> {
        let mut stream = res.bytes_stream();
        while let Some(item) = stream.next().await {
            let chunk = item.with_context(|| "Error while downloading file".to_string())?;
            writer
                .write_all(&chunk)
                .with_context(|| "Error while writing to file".to_string())?;
            pb.inc(chunk.len() as u64);
        }
        Ok(())
    }

    pub async fn download_file<P>(&self, url: &str, path: P) -> Result<CacheState>
    where
        P: AsRef<Path>,
    {
        let res = self.download_init(url, &path).await?;
        if res.is_none() {
            return Ok(CacheState::NoUpdate);
        }

        let (res, pb) = res.unwrap();
        let mut file = File::create(&path)
            .with_context(|| format!("Failed to create file '{}'", path.as_ref().display()))?;
        if url.ends_with(".xz") {
            self.download_internal(res, &pb, &mut XzDecoder::new(file))
                .await?;
        } else if url.ends_with(".gz") {
            let mut writer = flate2::write::GzDecoder::new(file);
            self.download_internal(res, &pb, &mut writer).await?;
            writer.try_finish()?;
        } else {
            self.download_internal(res, &pb, &mut file).await?;
        }
        pb.finish_with_message(format!("Downloaded {}", url));
        debug!("Download of {} to {:?} done", url, path.as_ref());
        Ok(CacheState::FreshFiles)
    }
}

#[derive(Debug)]
pub(crate) struct Cache {
    base_directory: BaseDirectories,
    downloader: Downloader,
}

impl Cache {
    pub fn new(force_download: bool) -> Result<Self> {
        Ok(Self {
            base_directory: BaseDirectories::with_prefix("Debian-RT-tools")?,
            downloader: Downloader::new(force_download),
        })
    }

    async fn download_excuses(&self) -> Result<CacheState> {
        self.downloader
            .download_file(
                "https://release.debian.org/britney/excuses.yaml",
                self.get_cache_path("excuses.yaml")?,
            )
            .await
    }

    async fn download_contents(&self, suite: Suite) -> Result<CacheState> {
        let mut state = CacheState::NoUpdate;
        for architecture in RELEASE_ARCHITECTURES
            .into_iter()
            .chain([Architecture::All].into_iter())
        {
            let url = format!(
                "https://deb.debian.org/debian/dists/{}/main/Contents-{}.gz",
                suite, architecture
            );
            let dest = format!("Contents_{}_{}", suite, architecture);
            if self
                .downloader
                .download_file(&url, self.get_cache_path(&dest)?)
                .await?
                == CacheState::FreshFiles
            {
                state = CacheState::FreshFiles;
            }
        }
        Ok(state)
    }

    async fn download_packages(&self) -> Result<CacheState> {
        let mut state = CacheState::NoUpdate;
        for architecture in RELEASE_ARCHITECTURES {
            let url = format!(
                "https://deb.debian.org/debian/dists/unstable/main/binary-{}/Packages.xz",
                architecture
            );
            let dest = format!("Packages_{}", architecture);
            if self
                .downloader
                .download_file(&url, self.get_cache_path(&dest)?)
                .await?
                == CacheState::FreshFiles
            {
                state = CacheState::FreshFiles;
            }
        }
        Ok(state)
    }

    async fn download_ftbfs_bugs(&self, codename: Codename) -> Result<CacheState> {
        let url = format!("https://udd.debian.org/bugs/?release={}&ftbfs=only&merged=ign&done=ign&rc=1&sortby=id&sorto=asc&format=yaml", codename);
        let dest = format!("udd-ftbfs-bugs-{}.yaml", codename);
        self.downloader
            .download_file(&url, self.get_cache_path(dest)?)
            .await
    }

    async fn download_auto_removals(&self) -> Result<CacheState> {
        self.downloader
            .download_file(
                "https://udd.debian.org/cgi-bin/autoremovals.yaml.cgi",
                self.get_cache_path("autoremovals.yaml")?,
            )
            .await
    }

    async fn download_outdated_builtusing(&self) -> Result<CacheState> {
        self.downloader
            .download_file(
                "https://ftp-master.debian.org/users/ansgar/outdated-built-using.txt",
                self.get_cache_path("outdated-built-using.txt")?,
            )
            .await
    }

    /*
    async fn download_sources(&self) -> Result<CacheState> {
        Ok(self
            .downloader
            .download_file(
                "https://deb.debian.org/debian/dists/unstable/main/source/Sources.xz",
                self.get_cache_path("Sources")?,
            )
            .await?)
    }
    */

    pub async fn download(&self, entries: &[CacheEntries]) -> Result<CacheState> {
        let mut state = CacheState::NoUpdate;
        for entry in entries {
            let new_state = match entry {
                CacheEntries::Excuses => self.download_excuses().await?,
                CacheEntries::Packages => self.download_packages().await?,
                // CacheEntries::Sources => self.download_sources().await?,
                CacheEntries::FTBFSBugs(codename) => self.download_ftbfs_bugs(*codename).await?,
                CacheEntries::AutoRemovals => self.download_auto_removals().await?,
                CacheEntries::OutdatedBuiltUsing => self.download_outdated_builtusing().await?,
                CacheEntries::Contents(suite) => self.download_contents(*suite).await?,
            };
            if new_state == CacheState::FreshFiles {
                state = CacheState::FreshFiles;
            }
        }
        Ok(state)
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

    pub fn get_package_paths(&self) -> Result<Vec<PathBuf>> {
        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES {
            all_paths.push(self.get_cache_path(format!("Packages_{}", architecture))?);
        }
        Ok(all_paths)
    }

    pub fn get_content_paths(&self, suite: Suite) -> Result<Vec<(Architecture, PathBuf)>> {
        let mut all_paths = vec![];
        for architecture in RELEASE_ARCHITECTURES
            .into_iter()
            .chain([Architecture::All].into_iter())
        {
            all_paths.push((
                architecture,
                self.get_cache_path(format!("Contents_{}_{}", suite, architecture))?,
            ));
        }
        Ok(all_paths)
    }
}
