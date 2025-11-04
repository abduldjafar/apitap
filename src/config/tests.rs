use std::fs::File;
use std::io::Write;
use std::env;

use crate::config::load_config_from_path;

// Helper to write YAML to a temp file and return its path
fn write_temp_yaml(contents: &str) -> std::path::PathBuf {
    let mut f = tempfile::NamedTempFile::new().expect("create temp file");
    write!(f, "{}", contents).expect("write temp yaml");
    f.into_temp_path().to_path_buf()
}

#[test]
fn test_config_load_fails_when_env_vars_missing() {
    // Ensure env vars are not set
    env::remove_var("TEST_PG_USER");
    env::remove_var("TEST_PG_PASS");

    let yaml = r#"
sources: []
targets:
  - name: postgres_sink
    type: postgres
    auth:
      username_env: TEST_PG_USER
      password_env: TEST_PG_PASS
    host: localhost
    database: testdb
"#;

    let path = write_temp_yaml(yaml);
    let res = load_config_from_path(path);
    assert!(res.is_err(), "expected config load to fail when env vars missing");
}

#[test]
fn test_config_load_succeeds_when_env_vars_present() {
    env::set_var("TEST_PG_USER", "alice");
    env::set_var("TEST_PG_PASS", "hunter2");

    let yaml = r#"
sources: []
targets:
  - name: postgres_sink
    type: postgres
    auth:
      username_env: TEST_PG_USER
      password_env: TEST_PG_PASS
    host: localhost
    database: testdb
"#;

    let path = write_temp_yaml(yaml);
    let res = load_config_from_path(path);
    assert!(res.is_ok(), "expected config load to succeed when env vars set");
}
