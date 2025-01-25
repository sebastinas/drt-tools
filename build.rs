// Copyright 2025 Sebastian Ramacher
// SPDX-License-Identifier: GPL-3.0-or-later

use {
    clap::CommandFactory,
    clap_complete::{Generator, Shell},
    std::{fs, io, path::Path},
};

include!("src/cli.rs");

fn write_completions_file<G: Generator>(generator: G, out_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(out_dir)?;
    let mut args = DrtToolsOptions::command();
    clap_complete::generate_to(generator, &mut args, "drt-tools", out_dir)
        .expect("clap complete generation failed");
    Ok(())
}

fn build_completion_scripts(out_dir: &Path) -> io::Result<()> {
    write_completions_file(
        Shell::Bash,
        &out_dir.join("bash-completion").join("completions"),
    )?;
    write_completions_file(
        Shell::Fish,
        &out_dir.join("fish").join("vendor-completions"),
    )?;
    write_completions_file(Shell::Zsh, &out_dir.join("zsh").join("site-functions"))?;
    Ok(())
}

fn build_man_page(out_dir: &Path) -> io::Result<()> {
    let out_dir = out_dir.join("man").join("man1");
    fs::create_dir_all(&out_dir)?;
    let cmd = DrtToolsOptions::command();
    clap_mangen::generate_to(cmd, out_dir)
}

fn main() -> io::Result<()> {
    // use PKGBUILD_OUT_DIR to better control output directory when packaging drt-tools
    let out_dir = if let Some(out_dir) = std::env::var_os("PKGBUILD_OUT_DIR") {
        out_dir
    } else {
        std::env::var_os("OUT_DIR").ok_or(io::ErrorKind::NotFound)?
    };
    let out_dir = PathBuf::from(out_dir);

    build_completion_scripts(&out_dir)?;
    build_man_page(&out_dir)
}
