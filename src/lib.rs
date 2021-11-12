use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{header, Client, Response, StatusCode};
use xz2::write::XzDecoder;

#[derive(Debug, Eq, PartialEq)]
pub enum CacheState {
    NoUpdate,
    FreshFiles,
}

#[derive(Debug)]
pub struct Downloader {
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
            return Ok(None);
        }

        let total_size = res
            .content_length()
            .ok_or_else(|| anyhow!("Failed to get content length from '{}'", &url))?;
        let pb = ProgressBar::new(total_size);
        pb.set_style(ProgressStyle::default_bar()
            .template("{msg}: {spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
            .progress_chars("█  "));
        pb.set_message(&format!("Downloading {}", url));
        Ok(Some((res, pb)))
    }

    async fn download_internal(
        &self,
        res: Response,
        pb: &ProgressBar,
        mut writer: impl Write,
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
        let file = File::create(&path)
            .with_context(|| format!("Failed to create file '{}'", path.as_ref().display()))?;
        if url.ends_with(".xz") {
            self.download_internal(res, &pb, XzDecoder::new(file))
                .await?;
        } else {
            self.download_internal(res, &pb, file).await?;
        }
        pb.finish_with_message(&format!("Downloaded {}", url));
        Ok(CacheState::FreshFiles)
    }
}
