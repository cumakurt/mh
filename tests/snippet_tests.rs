use mh::db::Database;

#[test]
fn saves_and_loads_snippets_from_database() {
    let temp_dir = mh::config::private_tempdir().expect("temp dir");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    database
        .save_snippet(
            "ssh-host",
            "ssh {{user}}@{{host}}",
            Some("Connect to a host"),
            Some("ssh"),
        )
        .expect("snippet should be saved");

    let snippet = database
        .get_snippet("ssh-host")
        .expect("snippet should load");
    assert_eq!(snippet.command, "ssh {{user}}@{{host}}");
    assert_eq!(snippet.description.as_deref(), Some("Connect to a host"));

    database
        .increment_snippet_use("ssh-host")
        .expect("snippet use should increment");
    let updated = database
        .get_snippet("ssh-host")
        .expect("snippet should reload");
    assert_eq!(updated.use_count, 1);
}
