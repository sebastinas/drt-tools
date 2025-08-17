// Copyright 2024 Sebastian Ramacher
// SPDX-License-Identifier: LGPL-3.0-or-later

use std::{fs::File, io::BufReader, path::PathBuf};

use assorted_debian_utils::release;

#[test]
fn parse_release_ramacher_unstable() {
    parse_release("Release-ramacher.at-unstable");
}

#[test]
fn parse_release_debian_unstable() {
    parse_release("Release-debian-unstable");
}

#[test]
fn parse_release_debian_forky() {
    parse_release("Release-debian-forky");
}

#[test]
fn parse_release_debian_trixie() {
    parse_release("Release-debian-trixie");
}

#[test]
fn parse_release_debian_bookworm() {
    parse_release("Release-debian-bookworm");
}

fn parse_release(data_file: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let release_path = manifest_dir.join("tests").join("data").join(data_file);

    let release_file = File::open(release_path).expect("Release file opened.");
    let archive =
        release::from_reader(BufReader::new(release_file)).expect("Release file parse correctly.");

    assert!(!archive.architectures.is_empty());
    assert!(!archive.components.is_empty());
    assert!(!archive.origin.is_empty());
    assert!(!archive.label.is_empty());
    assert!(!archive.files.is_empty());
}
