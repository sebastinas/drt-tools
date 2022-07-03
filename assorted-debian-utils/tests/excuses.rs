use std::{fs::File, io::BufReader, path::PathBuf};

use assorted_debian_utils::excuses;
use spectral::prelude::*;

#[test]
fn parse_excuses_2022_06_21() {
    parse_excuses("excuses-2022-06-21.yaml");
}

#[test]
fn parse_excuses_2022_07_02() {
    parse_excuses("excuses-2022-07-02.yaml");
}

fn parse_excuses(data_file: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let excuses_path = manifest_dir.join("tests").join("data").join(data_file);

    let excuses_file = File::open(excuses_path);
    asserting!("excuses file exists")
        .that(&excuses_file)
        .is_ok();

    let excuses = excuses::from_reader(BufReader::new(excuses_file.unwrap()));
    asserting!("excuses file parsed").that(&excuses).is_ok();
    let excuses = excuses.unwrap();

    asserting!("has excuses")
        .that(&excuses.sources.len())
        .is_not_equal_to(0);

    for source in excuses.sources {
        asserting!("item name contains source name")
            .that(&source.item_name)
            .contains(source.source.as_str());

        if source.is_binnmu() {
            asserting!("binNMU items have an associated architecture")
                .that(&source.binnmu_arch())
                .is_some();
        } else {
            asserting!("Non-binNMU items do not have an associated architecture")
                .that(&source.binnmu_arch())
                .is_none();
        }
    }
}
