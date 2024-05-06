// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use anyhow::{Context, Result};
use assorted_debian_utils::wb::{Error, WBCommand};
use openssh::{KnownHosts, Session, Stdio};
use tokio::io::AsyncWriteExt;

use crate::BaseOptions;

pub(crate) async fn execute_wb_commands<I>(commands: I, options: &BaseOptions) -> Result<()>
where
    I: IntoIterator<Item = WBCommand>,
{
    let iter = commands.into_iter();
    if options.dry_run {
        for command in iter {
            println!("{}", command);
        }
        return Ok(());
    }

    let session = Session::connect_mux(&options.buildd, KnownHosts::Strict).await?;

    let mut proc = session
        .command("wb")
        .stdin(Stdio::piped())
        .spawn()
        .await
        .context("Failed to spawn process")?;

    if let Some(mut stdin) = proc.stdin().take() {
        for command in iter {
            println!("{}", command);
            stdin
                .write_all(format!("{}\n", command).as_bytes())
                .await
                .with_context(|| format!("Failed to write wb command to stdin: {}", command))?;
        }
    } else {
        return Err(Error::ExecutionError).context("Unable to write to stdin.");
    }
    proc.wait_with_output()
        .await
        .context("Failed to wait on child process")?;
    Ok(())
}
