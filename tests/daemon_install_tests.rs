use std::fs;

use tempfile::tempdir;

#[test]
fn install_systemd_unit_writes_execstart() {
    let home = tempdir().expect("tempdir");
    let previous_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", home.path());
    }

    let install_result = mh::daemon::install_systemd_unit();
    if let Some(path) = previous_home {
        unsafe {
            std::env::set_var("HOME", path);
        }
    } else {
        unsafe {
            std::env::remove_var("HOME");
        }
    }
    install_result.expect("install unit");

    let unit = home
        .path()
        .join(".config/systemd/user/mh-record-daemon.service");
    let contents = fs::read_to_string(&unit).expect("read unit");
    assert!(contents.contains("ExecStart="));
    assert!(contents.contains("daemon run"));
    assert!(contents.contains("[Install]"));
}
