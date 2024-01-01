// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::{fs::File, io::BufReader, path::PathBuf};

use assorted_debian_utils::release;
use spectral::prelude::*;

#[test]
fn parse_release_ramacher_unstable() {
    parse_release("Release-ramacher.at-unstable");
}

#[test]
fn parse_release_debian_unstable() {
    parse_release("Release-debian-unstable");
}

#[test]
fn parse_release_debian_trixie() {
    parse_release("Release-debian-trixie");
}

#[test]
fn parse_release_debian_bookworm() {
    parse_release("Release-debian-bookworm");
}

#[test]
fn parse_release_debian_bullseye() {
    parse_release("Release-debian-bullseye");
}

fn parse_release(data_file: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let excuses_path = manifest_dir.join("tests").join("data").join(data_file);

    let excuses_file = File::open(excuses_path);
    asserting!("Release file exists")
        .that(&excuses_file)
        .is_ok();

    let archive = release::from_reader(BufReader::new(excuses_file.unwrap()));
    asserting!("Release file parsed").that(&archive).is_ok();
    let archive = archive.unwrap();

    asserting!("has architectures")
        .that(&archive.architectures.len())
        .is_not_equal_to(0);
    asserting!("has components")
        .that(&archive.components.len())
        .is_not_equal_to(0);
    asserting!("has Origin")
        .that(&archive.origin.len())
        .is_not_equal_to(0);
    asserting!("has Label")
        .that(&archive.label.len())
        .is_not_equal_to(0);
    asserting!("has Files")
        .that(&archive.files.len())
        .is_not_equal_to(0);
}
