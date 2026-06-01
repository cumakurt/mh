use std::fs;
use std::sync::{LazyLock, Mutex};

static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

#[test]
fn install_systemd_unit_writes_execstart() {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let home = mh::config::private_tempdir().expect("temp dir");
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
    assert!(contents.contains("ExecStart=\""));
    assert!(contents.contains("daemon run"));
    assert!(contents.contains("[Install]"));

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = fs::metadata(&unit)
            .expect("unit metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
}

#[test]
#[cfg(unix)]
fn install_systemd_unit_rejects_symlink_destination() {
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let home = mh::config::private_tempdir().expect("temp dir");
    let previous_home = std::env::var("HOME").ok();
    unsafe {
        std::env::set_var("HOME", home.path());
    }

    let unit_dir = home.path().join(".config/systemd/user");
    fs::create_dir_all(&unit_dir).expect("unit dir");
    let target = home.path().join("target.service");
    let link = unit_dir.join("mh-record-daemon.service");
    std::os::unix::fs::symlink(&target, &link).expect("symlink");

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

    let error = install_result.expect_err("symlink destination should be rejected");
    assert!(format!("{error:#}").contains("symlink"));
}
