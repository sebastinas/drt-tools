use std::{fs::File, io::BufReader, path::PathBuf};

use assorted_debian_utils::excuses;

#[test]
fn parse_excuses_2005_05_01() {
    parse_excuses("excuses-2024-11-30.yaml");
}

fn parse_excuses(data_file: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let excuses_path = manifest_dir.join("tests").join("data").join(data_file);
    let excuses_file = File::open(excuses_path).expect("Excuses file exists.");
    let excuses = excuses::from_reader(BufReader::new(excuses_file)).expect("Excuses file parsed.");

    assert!(!excuses.sources.is_empty());

    for source in excuses.sources {
        assert!(source.item_name.contains(source.source.as_ref()));

        if source.is_binnmu() {
            assert!(source.binnmu_arch().is_some());
        } else {
            assert!(source.binnmu_arch().is_none());
        }
    }
}
