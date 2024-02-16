// Copyright 2021-2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    borrow::Cow,
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use assorted_debian_utils::{
    architectures::Architecture,
    archive::{Codename, Extension, Suite},
    release,
};
use flate2::write::GzDecoder;
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
    Packages(Suite),
    Sources(Suite),
    FTBFSBugs(Codename),
    AutoRemovals,
    Contents(Suite),
    Release(Suite),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CacheState {
    NoUpdate,
    FreshFiles,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum Compressor {
    Xz,
    Gz,
    None,
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

    async fn download_init(
        &self,
        url: &str,
        path: &Path,
        mp: MultiProgress,
    ) -> Result<Option<(Response, ProgressBar)>> {
        debug!("Starting download of {} to {:?}", url, path);
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
        .and_then(|response| response.error_for_status())
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
            let pb = mp.add(ProgressBar::new_spinner());
            pb.set_message(format!("Downloading {}", url));
            Ok(Some((res, pb)))
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
            let chunk = item.with_context(|| "Error while downloading file")?;
            writer
                .write_all(&chunk)
                .with_context(|| "Error while writing to file")?;
            pb.inc(chunk.len() as u64);
        }
        Ok(())
    }

    pub async fn download_file<P>(
        &self,
        url: &str,
        path: P,
        compressor: Compressor,
        mp: MultiProgress,
    ) -> Result<CacheState>
    where
        P: AsRef<Path>,
    {
        self._download_file(url, path.as_ref(), compressor, mp)
            .await
    }

    async fn _download_file(
        &self,
        url: &str,
        path: &Path,
        compressor: Compressor,
        mp: MultiProgress,
    ) -> Result<CacheState> {
        let Some((res, pb)) = self.download_init(url, path, mp).await? else {
            return Ok(CacheState::NoUpdate);
        };

        let tmp_file = path.with_file_name({
            let mut tmp = path.file_name().unwrap().to_owned();
            tmp.push(".tmp");
            tmp
        });
        let mut file = File::create(&tmp_file)
            .with_context(|| format!("Failed to create temporary file '{:?}'", tmp_file))?;
        if compressor == Compressor::Xz {
            self.download_internal(res, &pb, &mut XzDecoder::new(file))
                .await?;
        } else if compressor == Compressor::Gz {
            let mut writer = GzDecoder::new(file);
            self.download_internal(res, &pb, &mut writer).await?;
            writer.try_finish()?;
        } else {
            self.download_internal(res, &pb, &mut file).await?;
        }
        pb.finish_with_message(format!("Downloaded {}", url));
        fs::rename(&tmp_file, path).with_context(|| {
            format!(
                "Failed to move temporary file '{:?}' to '{:?}'",
                tmp_file, path
            )
        })?;
        debug!("Download of {} to {:?} done", url, path);
        Ok(CacheState::FreshFiles)
    }
}

fn excuses_urls() -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
    vec![(
        "https://release.debian.org/britney/excuses.yaml.gz".into(),
        Compressor::Gz,
        "excuses.yaml".into(),
    )]
}

fn ftbfs_bugs_urls(codename: Codename) -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
    vec![(
        format!("https://udd.debian.org/bugs/?release={}&ftbfs=only&merged=ign&done=ign&rc=1&sortby=id&sorto=asc&format=yaml", codename).into(),
        Compressor::None,
        format!("udd-ftbfs-bugs-{}.yaml", codename).into()
    )]
}

fn auto_removals_urls() -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
    vec![(
        "https://udd.debian.org/cgi-bin/autoremovals.yaml.cgi".into(),
        Compressor::None,
        "autoremovals.yaml".into(),
    )]
}

#[derive(Debug)]
pub(crate) struct Cache {
    base_directory: BaseDirectories,
    downloader: Downloader,
    archive_mirror: String,
    unstable: release::Release,
    testing: release::Release,
    stable: release::Release,
    oldstable: release::Release,
    experimental: release::Release,
    stable_proposed_updates: release::Release,
    oldstable_proposed_updates: release::Release,
    stable_backports: release::Release,
    // oldstable_backports: release::Release,
}

fn empty_release() -> release::Release {
    release::Release {
        origin: Default::default(),
        label: Default::default(),
        suite: Suite::Unstable,
        codename: Codename::Sid,
        version: Default::default(),
        date: Default::default(),
        valid_until: Default::default(),
        acquire_by_hash: Default::default(),
        architectures: Default::default(),
        components: Default::default(),
        description: Default::default(),
        files: Default::default(),
    }
}

impl Cache {
    pub async fn new(force_download: bool, archive_mirror: &str) -> Result<Self> {
        let mut cache = Self {
            base_directory: BaseDirectories::with_prefix("Debian-RT-tools")?,
            downloader: Downloader::new(force_download),
            archive_mirror: archive_mirror.into(),
            unstable: empty_release(),
            testing: empty_release(),
            stable: empty_release(),
            oldstable: empty_release(),
            experimental: empty_release(),
            stable_proposed_updates: empty_release(),
            oldstable_proposed_updates: empty_release(),
            stable_backports: empty_release(),
            // oldstable_backports: empty_release(),
        };

        // download Release files for unstable, testing and stable
        cache
            .download(&[
                CacheEntries::Release(Suite::Unstable),
                CacheEntries::Release(Suite::Testing(None)),
                CacheEntries::Release(Suite::Stable(None)),
                CacheEntries::Release(Suite::OldStable(None)),
                CacheEntries::Release(Suite::Experimental),
                CacheEntries::Release(Suite::Stable(Some(Extension::ProposedUpdates))),
                CacheEntries::Release(Suite::OldStable(Some(Extension::ProposedUpdates))),
                CacheEntries::Release(Suite::Stable(Some(Extension::Backports))),
                // CacheEntries::Release(Suite::OldStable(Some(Extension::Backports))),
            ])
            .await?;

        cache.unstable = release::from_reader(
            cache.get_cache_bufreader(format!("Release_{}", Suite::Unstable))?,
        )?;
        cache.testing = release::from_reader(
            cache.get_cache_bufreader(format!("Release_{}", Suite::Testing(None)))?,
        )?;
        cache.stable = release::from_reader(
            cache.get_cache_bufreader(format!("Release_{}", Suite::Stable(None)))?,
        )?;
        cache.oldstable = release::from_reader(
            cache.get_cache_bufreader(format!("Release_{}", Suite::OldStable(None)))?,
        )?;
        cache.experimental = release::from_reader(
            cache.get_cache_bufreader(format!("Release_{}", Suite::Experimental))?,
        )?;
        cache.stable_proposed_updates =
            release::from_reader(cache.get_cache_bufreader(format!(
                "Release_{}",
                Suite::Stable(Some(Extension::ProposedUpdates))
            ))?)?;
        cache.oldstable_proposed_updates =
            release::from_reader(cache.get_cache_bufreader(format!(
                "Release_{}",
                Suite::OldStable(Some(Extension::ProposedUpdates))
            ))?)?;
        cache.stable_backports = release::from_reader(cache.get_cache_bufreader(format!(
            "Release_{}",
            Suite::Stable(Some(Extension::Backports))
        ))?)?;
        // cache.oldstable_backports = release::from_reader(cache.get_cache_bufreader(format!(
        //     "Release_{}",
        //     Suite::OldStable(Some(Extension::Backports))
        // ))?)?;

        Ok(cache)
    }

    fn lookup_url(&self, suite: Suite, path: &str) -> String {
        format!(
            "{}/dists/{}/{}",
            self.archive_mirror,
            suite,
            match suite {
                Suite::Unstable => &self.unstable,
                Suite::Testing(_) => &self.testing,
                Suite::Stable(None) => &self.stable,
                Suite::OldStable(None) => &self.oldstable,
                Suite::Experimental => &self.experimental,
                Suite::Stable(Some(Extension::ProposedUpdates)) => &self.stable_proposed_updates,
                Suite::OldStable(Some(Extension::ProposedUpdates)) =>
                    &self.oldstable_proposed_updates,
                Suite::Stable(Some(Extension::Backports)) => &self.stable_backports,
                // Suite::OldStable(Some(Extension::Backports)) => &self.oldstable_backports,
                _ => unreachable!("Suite {} is currently not handled.", suite),
            }
            .lookup_url(path)
            .expect("file needs to be available")
        )
    }

    fn contents_urls(
        &self,
        suite: Suite,
    ) -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
        self.architectures_for_suite(suite)
            .into_iter()
            .map(|architecture| {
                (
                    self.lookup_url(suite, &format!("main/Contents-{}.gz", architecture))
                        .into(),
                    Compressor::Gz,
                    format!("Contents_{}_{}", suite, architecture).into(),
                )
            })
            .collect()
    }

    fn packages_urls(
        &self,
        suite: Suite,
    ) -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
        self.architectures_for_suite(suite)
            .into_iter()
            .map(|architecture| {
                (
                    self.lookup_url(suite, &format!("main/binary-{}/Packages.xz", architecture))
                        .into(),
                    Compressor::Xz,
                    format!("Packages_{}_{}", suite, architecture).into(),
                )
            })
            .collect()
    }

    fn source_urls(&self, suite: Suite) -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
        vec![(
            self.lookup_url(suite, "main/source/Sources.xz").into(),
            Compressor::Xz,
            format!("Sources_{}", suite).into(),
        )]
    }

    fn release_urls(
        &self,
        suite: Suite,
    ) -> Vec<(Cow<'static, str>, Compressor, Cow<'static, str>)> {
        vec![(
            format!("{}/dists/{}/Release", self.archive_mirror, suite).into(),
            Compressor::None,
            format!("Release_{}", suite).into(),
        )]
    }

    fn cache_entries_to_urls_dests(
        &self,
        entries: &[CacheEntries],
    ) -> Result<Vec<(Cow<'static, str>, Compressor, PathBuf)>> {
        entries
            .iter()
            .flat_map(|entry| {
                match entry {
                    CacheEntries::Excuses => excuses_urls(),
                    CacheEntries::Packages(suite) => self.packages_urls(*suite),
                    CacheEntries::Sources(suite) => self.source_urls(*suite),
                    CacheEntries::FTBFSBugs(codename) => ftbfs_bugs_urls(*codename),
                    CacheEntries::AutoRemovals => auto_removals_urls(),
                    CacheEntries::Contents(suite) => self.contents_urls(*suite),
                    CacheEntries::Release(suite) => self.release_urls(*suite),
                }
                .into_iter()
            })
            .map(|(url, compressor, dest)| {
                Ok((url, compressor, self.get_cache_path(dest.as_ref())?))
            })
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
            .map(|(url, compressor, dest)| {
                let downloader = self.downloader.clone();
                let mp = mp.clone();
                tokio::spawn(async move {
                    debug!("Starting task to download {}", url);
                    downloader.download_file(&url, dest, compressor, mp).await
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

    pub fn get_package_paths(&self, suite: Suite, with_all: bool) -> Result<Vec<PathBuf>> {
        let mut all_paths = vec![];
        for architecture in self.architectures_for_suite(suite) {
            if !with_all && architecture == Architecture::All {
                continue;
            }

            all_paths.push(self.get_cache_path(format!("Packages_{}_{}", suite, architecture))?);
        }
        Ok(all_paths)
    }

    pub fn get_source_path(&self, suite: Suite) -> Result<PathBuf> {
        self.get_cache_path(format!("Sources_{}", suite))
    }

    pub fn get_content_paths(&self, suite: Suite) -> Result<Vec<(Architecture, PathBuf)>> {
        let mut all_paths = vec![];
        for architecture in self.architectures_for_suite(suite) {
            all_paths.push((
                architecture,
                self.get_cache_path(format!("Contents_{}_{}", suite, architecture))?,
            ));
        }
        Ok(all_paths)
    }

    // Architectures for a suite (including Arch: all)
    pub fn architectures_for_suite(&self, suite: Suite) -> Vec<Architecture> {
        match suite {
            Suite::Unstable | Suite::Experimental => self.unstable.architectures.clone(),
            Suite::Testing(_) => self.testing.architectures.clone(),
            Suite::Stable(_) => self.stable.architectures.clone(),
            Suite::OldStable(_) => self.oldstable.architectures.clone(),
        }
    }
}

pub fn source_skip_binnmu(source: &str) -> bool {
    // skip some packages that make no sense to binNMU
    source.starts_with("debian-installer")
        || source == "linux"
        || (source.contains("-signed")
            && (source.starts_with("grub-")
                || source.starts_with("linux-")
                || source.starts_with("shim-")
                || source.starts_with("fwupd-")))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn skip_binnmu() {
        assert!(source_skip_binnmu("linux-signed-i386"));
        assert!(!source_skip_binnmu("zathura-signed-foo"));
    }
}
