// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use anyhow::{Context, Result};
use assorted_debian_utils::wb::{Error, WBCommand};
use openssh::{KnownHosts, Session, Stdio};
use tokio::{io::AsyncWriteExt, task::JoinSet};

pub async fn execute_wb_commands<I>(commands: I, dry_run: bool) -> Result<()>
where
    I: IntoIterator<Item = WBCommand>,
{
    let iter = commands.into_iter();
    if dry_run {
        for command in iter {
            println!("{}", command);
        }
        return Ok(());
    }

    let session = Arc::new(Session::connect_mux("wuiet.debian.org", KnownHosts::Strict).await?);

    let mut tasks = JoinSet::new();
    for command in iter {
        println!("{}", command);

        let mut proc = session
            .clone()
            .arc_command("wb")
            .stdin(Stdio::piped())
            .spawn()
            .await
            .context("Failed to spawn process")?;

        tasks.spawn(async move {
            if let Some(mut stdin) = proc.stdin().take() {
                stdin
                    .write_all(command.to_string().as_bytes())
                    .await
                    .context("Failed to write to stdin")?;
            } else {
                return Err(Error::ExecutionError).context("Unable to write to stdin.");
            }
            proc.wait_with_output()
                .await
                .context("Failed to wait on child process")?;
            Ok(())
        });
    }

    let mut ret: Result<()> = Ok(());
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok(_)) => continue,
            Ok(e) => ret = e,
            Err(e) => ret = Err(e).context("Failed to join task"),
        }
    }
    ret
}
