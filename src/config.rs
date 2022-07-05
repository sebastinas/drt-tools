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
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{debug, trace};
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
    Contents(Suite),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CacheState {
    NoUpdate,
    FreshFiles,
}

#[derive(Debug, Clone)]
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

    async fn download_init<P>(
        &self,
        url: &str,
        path: P,
        mp: MultiProgress,
    ) -> Result<Option<(Response, ProgressBar)>>
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
            let pb = mp.add(ProgressBar::new(total_size));
            pb.set_style(default_progress_style()
            .template("{msg}: {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")?
            );
            pb.set_message(format!("Downloading {}", url));
            Ok(Some((res, pb)))
        } else {
            Ok(Some((res, mp.add(ProgressBar::hidden()))))
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

    pub async fn download_file<P>(
        &self,
        url: &str,
        path: P,
        mp: MultiProgress,
    ) -> Result<CacheState>
    where
        P: AsRef<Path>,
    {
        let (res, pb) = match self.download_init(url, &path, mp).await? {
            None => return Ok(CacheState::NoUpdate),
            Some(val) => val,
        };

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

    fn excuses_urls(&self) -> Vec<(String, String)> {
        vec![(
            "https://release.debian.org/britney/excuses.yaml".into(),
            "excuses.yaml".into(),
        )]
    }

    fn contents_urls(&self, suite: Suite) -> Vec<(String, String)> {
        RELEASE_ARCHITECTURES
            .into_iter()
            .chain([Architecture::All].into_iter())
            .map(|architecture| {
                (
                    format!(
                        "https://deb.debian.org/debian/dists/{}/main/Contents-{}.gz",
                        suite, architecture
                    ),
                    format!("Contents_{}_{}", suite, architecture),
                )
            })
            .collect()
    }

    fn packages_urls(&self) -> Vec<(String, String)> {
        RELEASE_ARCHITECTURES
            .into_iter()
            .map(|architecture| {
                (
                    format!(
                        "https://deb.debian.org/debian/dists/unstable/main/binary-{}/Packages.xz",
                        architecture
                    ),
                    format!("Packages_{}", architecture),
                )
            })
            .collect()
    }

    fn ftbfs_bugs_urls(&self, codename: Codename) -> Vec<(String, String)> {
        vec![(
            format!("https://udd.debian.org/bugs/?release={}&ftbfs=only&merged=ign&done=ign&rc=1&sortby=id&sorto=asc&format=yaml", codename),
            format!("udd-ftbfs-bugs-{}.yaml", codename)
        )]
    }

    fn auto_removals_urls(&self) -> Vec<(String, String)> {
        vec![(
            "https://udd.debian.org/cgi-bin/autoremovals.yaml.cgi".into(),
            "autoremovals.yaml".into(),
        )]
    }

    fn outdateed_builtusing_urls(&self) -> Vec<(String, String)> {
        vec![(
            "https://ftp-master.debian.org/users/ansgar/outdated-built-using.txt".into(),
            "outdated-built-using.txt".into(),
        )]
    }

    fn cache_entries_to_urls_dests(
        &self,
        entries: &[CacheEntries],
    ) -> Result<Vec<(String, PathBuf)>> {
        entries
            .iter()
            .flat_map(|entry| {
                match entry {
                    CacheEntries::Excuses => self.excuses_urls(),
                    CacheEntries::Packages => self.packages_urls(),
                    CacheEntries::FTBFSBugs(codename) => self.ftbfs_bugs_urls(*codename),
                    CacheEntries::AutoRemovals => self.auto_removals_urls(),
                    CacheEntries::OutdatedBuiltUsing => self.outdateed_builtusing_urls(),
                    CacheEntries::Contents(suite) => self.contents_urls(*suite),
                }
                .into_iter()
            })
            .map(|(url, dest)| Ok((url, self.get_cache_path(dest)?)))
            .collect()
    }

    pub async fn download(&self, entries: &[CacheEntries]) -> Result<CacheState> {
        let urls_and_dests = self.cache_entries_to_urls_dests(entries)?;
        trace!(
            "Scheduling {} URLs to download: {:?}",
            urls_and_dests.len(),
            urls_and_dests
        );

        let mp = MultiProgress::new();
        let join_handles: Vec<_> = urls_and_dests
            .into_iter()
            .map(|(url, dest)| {
                let downloader = self.downloader.clone();
                let mp = mp.clone();
                tokio::spawn(async move {
                    debug!("Starting task to download {}", url);
                    downloader.download_file(&url, dest, mp).await
                })
            })
            .collect();

        let mut state = Ok(CacheState::NoUpdate);
        for handle in join_handles {
            match handle.await {
                Ok(download_result) => match download_result {
                    Ok(CacheState::FreshFiles) => {
                        if state.is_ok() {
                            state = Ok(CacheState::FreshFiles);
                        }
                    }
                    Err(err) => state = Err(err),
                    _ => {}
                },
                Err(err) => state = Err(err.into()),
            };
        }
        state
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
