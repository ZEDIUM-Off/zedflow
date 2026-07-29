use zedflow_coding_agent::utils::changelog::{ChangelogEntry, compare_versions, get_new_entries};

#[test]
fn filters_entries_newer_than_the_recorded_version() {
    let entries = vec![
        ChangelogEntry {
            major: 1,
            minor: 0,
            patch: 0,
            content: "old".into(),
        },
        ChangelogEntry {
            major: 1,
            minor: 2,
            patch: 0,
            content: "new".into(),
        },
    ];
    assert_eq!(compare_versions(&entries[0], &entries[1]), -1);
    assert_eq!(get_new_entries(&entries, "1.0.0"), vec![entries[1].clone()]);
}
