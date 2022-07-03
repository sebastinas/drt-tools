use std::{fs::File, io::BufReader, path::PathBuf};

use assorted_debian_utils::excuses;
use spectral::prelude::*;

#[test]
fn parse_excuses() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let excuses_path = manifest_dir
        .join("tests")
        .join("data")
        .join("excuses-2022-07-02.yaml");

    let excuses_file = File::open(excuses_path);
    asserting!("excuses file exists")
        .that(&excuses_file)
        .is_ok();

    let excuses = excuses::from_reader(BufReader::new(excuses_file.unwrap()));
    asserting!("excuses file parsed").that(&excuses).is_ok();
    let excuses = excuses.unwrap();

    asserting!("excuses timestap matches")
        .that(
            &excuses
                .generated_date
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
        )
        .is_equal_to(String::from("2022-07-02 20:09:06"));
    asserting!("has excuses")
        .that(&excuses.sources.len())
        .is_not_equal_to(0);
}
