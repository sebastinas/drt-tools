use std::{collections::HashMap, fs::File, io::BufReader};

use anyhow::Result;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct UDDBug {
    pub(crate) id: u32,
    pub(crate) source: String,
}

pub(crate) fn load_bugs_from_reader(reader: BufReader<File>) -> Result<Vec<UDDBug>> {
    serde_yaml::from_reader(reader).map_err(|e| e.into())
}

pub(crate) fn load_hashmap_bugs_from_reader(
    reader: BufReader<File>,
) -> Result<HashMap<String, u32>> {
    let bugs: Vec<UDDBug> = load_bugs_from_reader(reader)?;
    Ok(bugs.into_iter().map(|bug| (bug.source, bug.id)).collect())
}
