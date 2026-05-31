use mh::classifier::classify_command;
use mh::config::AppConfig;

#[test]
fn classifies_known_command_prefixes() {
    let config = AppConfig::default();

    assert_eq!(
        classify_command("git status", &config.categories),
        Some("git".to_string())
    );
    assert_eq!(
        classify_command("docker ps -a", &config.categories),
        Some("docker".to_string())
    );
    assert_eq!(
        classify_command("curl https://example.test", &config.categories),
        Some("network".to_string())
    );
}

#[test]
fn leaves_unknown_commands_uncategorized() {
    let config = AppConfig::default();

    assert_eq!(classify_command("echo hello", &config.categories), None);
}
