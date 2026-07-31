use std::time::{Duration, UNIX_EPOCH};

use zedflow_coding_agent::session_manager::session_modified_timestamp;

#[test]
fn session_modified_timestamp_uses_the_last_message_not_file_mtime() {
    let file_mtime = UNIX_EPOCH + Duration::from_secs(99);
    let message_time = UNIX_EPOCH + Duration::from_secs(7);

    assert_eq!(
        session_modified_timestamp(file_mtime, [Some(message_time)]),
        message_time
    );
    assert_eq!(
        session_modified_timestamp(file_mtime, std::iter::empty()),
        file_mtime
    );
}
