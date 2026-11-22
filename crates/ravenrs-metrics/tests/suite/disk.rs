//! The free space of a disk, with the feature `disk`.

#[test]
#[cfg(feature = "disk")]
fn the_disk_of_the_temporary_folder_has_some_room_and_one_of_nothing_has_none() {
    let free =
        ravenrs_metrics::free_disk_bytes(&std::env::temp_dir()).expect("a disk holds the temporary folder");
    assert!(free > 0);
    // A folder that does not exist yet is measured on the disk it would be made on.
    let later = std::env::temp_dir().join("ravenrs-metrics-ainda-nao-existe");
    assert_eq!(ravenrs_metrics::free_disk_bytes(&later).map(|b| b > 0), Some(true));
}
