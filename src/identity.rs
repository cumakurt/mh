/// Returns true when the current process has root privileges.
pub fn is_effective_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        whoami::username() == "root"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_root_matches_platform_expectation() {
        #[cfg(unix)]
        {
            let expected = unsafe { libc::geteuid() == 0 };
            assert_eq!(is_effective_root(), expected);
        }
        #[cfg(not(unix))]
        {
            let _ = is_effective_root();
        }
    }
}
