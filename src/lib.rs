use std::fs;
use std::fs::File;
use std::io::Write;

use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{Client, Response};
use xz2::write::XzDecoder;

#[derive(Debug, Eq, PartialEq)]
pub enum CacheState {
    NoUpdate,
    FreshFiles,
}

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

    async fn download_file_init(
        &self,
        url: &str,
        path: &str,
    ) -> Result<Option<(Response, ProgressBar)>> {
        let res = if let Ok(dst_metadata) = fs::metadata(path) {
            let date = dst_metadata.modified()?;
            let res = self.client.get(url);
            if !self.always_download {
                res.header(
                    reqwest::header::IF_MODIFIED_SINCE,
                    httpdate::fmt_http_date(date),
                )
            } else {
                res
            }
        } else {
            self.client.get(url)
        }
        .send()
        .await
        .with_context(|| format!("Failed to GET from '{}'", &url))?;

        if !self.always_download && res.status() == reqwest::StatusCode::NOT_MODIFIED {
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

    pub async fn download_file(&self, url: &str, path: &str) -> Result<CacheState> {
        let res = self.download_file_init(url, path).await?;
        if let None = res {
            return Ok(CacheState::NoUpdate);
        }
        let (res, pb) = res.unwrap();

        let mut file =
            File::create(path).with_context(|| format!("Failed to create file '{}'", path))?;
        let mut stream = res.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item.with_context(|| "Error while downloading file".to_string())?;
            file.write_all(&chunk)
                .with_context(|| "Error while writing to file".to_string())?;
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message(&format!("Downloaded {}", url));
        Ok(CacheState::FreshFiles)
    }

    pub async fn download_file_unxz(&self, url: &str, path: &str) -> Result<CacheState> {
        let res = self.download_file_init(url, path).await?;
        if let None = res {
            return Ok(CacheState::NoUpdate);
        }
        let (res, pb) = res.unwrap();

        let mut file = XzDecoder::new(
            File::create(path).with_context(|| format!("Failed to create file '{}'", path))?,
        );
        let mut stream = res.bytes_stream();

        while let Some(item) = stream.next().await {
            let chunk = item.with_context(|| "Error while downloading file".to_string())?;
            file.write_all(&chunk)
                .with_context(|| "Error while writing to file".to_string())?;
            pb.inc(chunk.len() as u64);
        }

        pb.finish_with_message(&format!("Downloaded {}", url));
        Ok(CacheState::FreshFiles)
    }
}
