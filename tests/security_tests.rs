use mh::config::AppConfig;
use mh::security::{SecurityAction, contains_secret, mask_secrets, process_command};

mod common;
use common::IsolatedConfigHome;

fn assert_masked(command: &str, secret: &str) {
    let _guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let decision = process_command(command, &config).expect("security processing should succeed");
    assert_eq!(
        decision.action,
        SecurityAction::Masked,
        "expected masking for: {command}"
    );
    assert!(
        decision.command.contains("****"),
        "expected redaction marker in: {}",
        decision.command
    );
    assert!(
        !decision.command.contains(secret),
        "secret leaked in masked command: {}",
        decision.command
    );
}

#[test]
fn masks_mysql_password_with_space() {
    assert_masked("mysql -u root -p Secret123", "Secret123");
}

#[test]
fn masks_curl_long_user_flag() {
    assert_masked(
        "curl --user admin:secret123 https://api.example.com",
        "secret123",
    );
}

#[test]
fn masks_wget_password_flag() {
    assert_masked(
        "wget --password=secret123 https://example.com/file",
        "secret123",
    );
}

#[test]
fn masks_redis_auth_flag() {
    assert_masked("redis-cli -a topsecret keys '*'", "topsecret");
}

#[test]
fn masks_database_connection_url() {
    assert_masked(
        "psql postgresql://admin:secret123@db.example.com/app",
        "secret123",
    );
}

#[test]
fn skips_leading_tab_prefixed_command() {
    let _guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let decision =
        process_command("\techo hidden", &config).expect("security processing should succeed");
    assert!(matches!(decision.action, SecurityAction::Skipped(_)));
}

#[test]
fn masks_mysql_inline_password() {
    assert_masked("mysql -u root -pSecret123", "Secret123");
}

#[test]
fn masks_curl_bearer_token() {
    assert_masked(
        r#"curl -H "Authorization: Bearer abc123" https://api.example.com"#,
        "abc123",
    );
}

#[test]
fn masks_aws_secret_export() {
    let _guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let decision = process_command("export AWS_SECRET_ACCESS_KEY=xxxx", &config)
        .expect("security processing should succeed");
    assert_eq!(decision.action, SecurityAction::Masked);
    assert!(decision.command.contains("****"));
    assert!(!decision.command.contains("xxxx"));
}

#[test]
fn masks_sshpass_password() {
    assert_masked("sshpass -p password ssh root@1.1.1.1", "password");
}

#[test]
fn masks_docker_login_password() {
    assert_masked("docker login -u user -p password", "password");
}

#[test]
fn masks_kubectl_token_flag() {
    assert_masked("kubectl config set-credentials user --token=abc", "abc");
}

#[test]
fn masks_bearer_token_by_default() {
    let _guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let decision = process_command(
        r#"curl -H "Authorization: Bearer abc123" https://example.test"#,
        &config,
    )
    .expect("security processing should succeed");

    assert_eq!(decision.action, SecurityAction::Masked);
    assert!(decision.command.contains("Authorization: Bearer ****"));
    assert!(!decision.command.contains("abc123"));
}

#[test]
fn can_skip_secret_commands() {
    let _guard = IsolatedConfigHome::new();
    let mut config = AppConfig::default();
    config.security.skip_secret_commands = true;

    let decision = process_command("export GITHUB_TOKEN=ghp_secret", &config)
        .expect("security processing should succeed");

    assert!(matches!(decision.action, SecurityAction::Skipped(_)));
}

#[test]
fn ignores_configured_exact_commands() {
    let config = AppConfig::default();
    let decision = process_command("clear", &config).expect("security processing should succeed");

    assert!(matches!(decision.action, SecurityAction::Skipped(_)));
}

#[test]
fn masks_mariadb_inline_password() {
    assert_masked("mariadb -u root -pSecret123", "Secret123");
}

#[test]
fn masks_curl_user_password() {
    assert_masked(
        "curl -u admin:secret123 https://api.example.com",
        "secret123",
    );
}

#[test]
fn masks_sshpass_env_variable() {
    assert_masked("SSHPASS=topsecret sshpass -e ssh host", "topsecret");
}

#[test]
fn redact_for_audit_hides_private_mode_content() {
    let guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let marker = guard.config_dir().join("private");
    std::fs::write(&marker, "1").expect("private marker");
    let redacted = mh::security::redact_for_audit("mysql -pSecret123", &config).expect("redact");
    assert!(!redacted.contains("Secret123"));
    assert!(redacted.contains("private mode"));
}

#[test]
fn skips_secret_when_masking_disabled() {
    let _guard = IsolatedConfigHome::new();
    let mut config = AppConfig::default();
    config.security.mask_secrets = false;
    config.security.skip_secret_commands = false;

    let decision = process_command("mysql -u root -pSecret123", &config)
        .expect("security processing should succeed");
    assert!(matches!(decision.action, SecurityAction::Skipped(_)));
}

#[test]
fn masks_pgpassword_env() {
    assert_masked("PGPASSWORD=secret psql -c 'select 1'", "secret");
}

#[test]
fn does_not_flag_grep_uppercase_p_flag() {
    assert!(
        !contains_secret("grep -P 'pattern' file.txt").expect("contains_secret should succeed"),
        "grep -P should not be treated as a credential flag"
    );
    let masked = mask_secrets("grep -P 'pattern' file.txt").expect("mask_secrets should succeed");
    assert_eq!(masked, "grep -P 'pattern' file.txt");
}

#[test]
fn does_not_flag_benign_password_mentions() {
    assert!(
        !contains_secret("echo password policy documentation")
            .expect("contains_secret should succeed")
    );
    assert!(!contains_secret("grep -r token README.md").expect("contains_secret should succeed"));
    assert!(!contains_secret("man sshpass").expect("contains_secret should succeed"));
}

#[test]
fn still_detects_exported_aws_secret_without_broad_keyword_match() {
    assert!(
        contains_secret("export AWS_SECRET_ACCESS_KEY=xxxx")
            .expect("contains_secret should succeed")
    );
}

#[test]
fn masks_aws_secret_without_export_prefix() {
    assert_masked("AWS_SECRET_ACCESS_KEY=xxxx cmd", "xxxx");
}

#[test]
fn does_not_flag_psql_port_flag() {
    assert!(
        !contains_secret("psql -p 5432 -h localhost -U admin app")
            .expect("contains_secret should succeed"),
        "psql -p is a port flag, not a password"
    );
    let masked = mask_secrets("psql -p 5432 -h localhost -U admin app")
        .expect("mask_secrets should succeed");
    assert_eq!(masked, "psql -p 5432 -h localhost -U admin app");
}

#[test]
fn masks_quoted_mysql_password() {
    assert_masked("mysql -u root -p 'Secret 123'", "Secret 123");
}

#[test]
fn masks_quoted_sshpass_password() {
    assert_masked("sshpass -p 'my pass' ssh root@1.1.1.1", "my pass");
}

#[test]
fn masks_authorization_basic_header() {
    assert_masked(
        r#"curl -H "Authorization: Basic dXNlcjpzZWNyZXQ=" https://api.example.com"#,
        "dXNlcjpzZWNyZXQ=",
    );
}

#[test]
fn masks_kubectl_token_with_space() {
    assert_masked(
        "kubectl config set-credentials user --token abc123",
        "abc123",
    );
}

#[test]
fn skips_recording_when_private_mode_env_is_set() {
    let guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let env_name = config.security.private_mode_env.clone();
    unsafe {
        std::env::set_var(&env_name, "1");
    }
    let decision =
        process_command("echo private", &config).expect("security processing should succeed");
    assert!(matches!(decision.action, SecurityAction::Skipped(_)));
    unsafe {
        std::env::remove_var(&env_name);
    }
    let _ = guard;
}

#[test]
fn skips_recording_when_private_mode_marker_exists() {
    let guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let marker = guard.config_dir().join("private");
    std::fs::write(&marker, "1").expect("private marker");
    let decision =
        process_command("echo private", &config).expect("security processing should succeed");
    assert!(matches!(decision.action, SecurityAction::Skipped(_)));
}

#[test]
fn break_glass_overrides_private_mode() {
    let guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let marker = guard.config_dir().join("private");
    std::fs::write(&marker, "1").expect("private marker");
    mh::break_glass::activate("incident response", 1).expect("break-glass activate");
    let decision =
        process_command("echo break-glass", &config).expect("security processing should succeed");
    assert_eq!(decision.action, SecurityAction::Store);
    mh::break_glass::deactivate().expect("break-glass deactivate");
}

#[test]
fn does_not_flag_unrelated_u_flag() {
    assert!(
        !contains_secret("some-tool -u admin:secret https://example.com")
            .expect("contains_secret should succeed"),
        "curl-scoped -u detection should not match unrelated tools"
    );
}

#[test]
fn masks_kubectl_from_literal() {
    assert_masked(
        "kubectl create secret generic app --from-literal=token=abc123",
        "abc123",
    );
}

#[test]
fn does_not_flag_random_long_numeric_id() {
    assert!(
        !contains_secret("echo order-id 1234567890123456 placeholder").expect("contains_secret"),
        "non-Luhn numeric strings should not be treated as credit cards"
    );
}

#[test]
fn skips_or_masks_inline_private_key_pem() {
    let pem = "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQ\n-----END OPENSSH PRIVATE KEY-----";
    let command = format!("cat <<'EOF' > id_ed25519\n{pem}\nEOF");
    assert!(
        contains_secret(&command).expect("contains_secret"),
        "inline PEM private keys must be treated as secrets"
    );
    let _guard = IsolatedConfigHome::new();
    let config = AppConfig::default();
    let decision = process_command(&command, &config).expect("security processing should succeed");
    assert!(
        matches!(
            decision.action,
            SecurityAction::Masked | SecurityAction::Skipped(_)
        ),
        "PEM material must not be stored verbatim"
    );
    assert!(!decision.command.contains("b3BlbnNzaC1rZXk"));
}

#[test]
fn masks_npm_config_auth_token() {
    assert_masked(
        "npm_config_//registry.npmjs.org/:_authToken=npm_secret_token",
        "npm_secret_token",
    );
}

#[test]
fn masks_helm_set_secret_value() {
    assert_masked(
        "helm upgrade app chart --set secret.password=topsecret",
        "topsecret",
    );
}

#[test]
fn critical_secret_command_regression_suite() {
    let cases = [
        ("mysql -u root -pSecret123", "Secret123"),
        (
            r#"curl -H "Authorization: Bearer abc123" https://api.example.com"#,
            "abc123",
        ),
        ("export AWS_SECRET_ACCESS_KEY=xxxx", "xxxx"),
        ("sshpass -p password ssh root@1.1.1.1", "password"),
        ("docker login -u user -p password", "password"),
        ("kubectl config set-credentials user --token=abc", "abc"),
    ];
    for (command, secret) in cases {
        assert_masked(command, secret);
    }
}
