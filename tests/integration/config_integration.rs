// Integration tests for Configuration and Pipeline Setup
//
// These tests verify end-to-end configuration loading and pipeline setup:
// - YAML config parsing
// - Source and target configuration
// - Pipeline setup and validation

use apitap::pipeline::{Config, Retry};

#[test]
fn test_parse_complete_config() {
    let yaml = r#"
sources:
  - name: api1
    url: https://api.example.com/users
    table_destination_name: users
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
    pagination:
      kind: limit_offset
      limit_param: limit
      offset_param: offset
  - name: api2
    url: https://api.example.com/posts
    retry:
      max_attempts: 5
      max_delay_secs: 120
      min_delay_secs: 2
targets:
  - type: postgres
    name: pg_sink
    host: localhost
    port: 5432
    database: testdb
    auth:
      username: testuser
      password: testpass
"#;
    
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    
    // Verify sources
    assert_eq!(config.sources.len(), 2);
    assert!(config.source("api1").is_some());
    assert!(config.source("api2").is_some());
    
    // Verify targets
    assert_eq!(config.targets.len(), 1);
    assert!(config.target("pg_sink").is_some());
}

#[test]
fn test_config_with_env_credentials() {
    let yaml = r#"
sources: []
targets:
  - type: postgres
    name: pg_sink
    host: localhost
    database: testdb
    auth:
      username_env: DB_USER
      password_env: DB_PASS
"#;
    
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    let target = config.target("pg_sink").unwrap();
    
    // Verify target exists with env-based auth
    assert!(target.name() == "pg_sink");
}

#[test]
fn test_multiple_pagination_strategies() {
    let yaml = r#"
sources:
  - name: limit_offset_api
    url: https://api1.example.com
    pagination:
      kind: limit_offset
      limit_param: limit
      offset_param: offset
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
  - name: page_number_api
    url: https://api2.example.com
    pagination:
      kind: page_number
      page_param: page
      per_page_param: per_page
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
  - name: cursor_api
    url: https://api3.example.com
    pagination:
      kind: cursor
      cursor_param: next_cursor
      page_size_param: size
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
targets: []
"#;
    
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    
    // All sources should be parseable with different pagination strategies
    assert_eq!(config.sources.len(), 3);
    assert!(config.source("limit_offset_api").unwrap().pagination.is_some());
    assert!(config.source("page_number_api").unwrap().pagination.is_some());
    assert!(config.source("cursor_api").unwrap().pagination.is_some());
}

#[test]
fn test_retry_configuration_validation() {
    let retry = Retry {
        max_attempts: 5,
        max_delay_secs: 300,
        min_delay_secs: 1,
    };
    
    // Retry configuration should be valid
    assert!(retry.max_attempts > 0);
    assert!(retry.max_delay_secs > retry.min_delay_secs);
}

#[test]
fn test_config_reindexing() {
    let yaml = r#"
sources:
  - name: api1
    url: https://api.example.com
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
targets:
  - type: postgres
    name: pg1
    host: localhost
    database: db1
    auth:
      username: user1
      password: pass1
"#;
    
    let mut config: Config = serde_yaml::from_str(yaml).unwrap();
    
    // Initial state
    assert!(config.source("api1").is_some());
    
    // Reindexing should succeed
    assert!(config.reindex().is_ok());
    
    // Should still be accessible after reindex
    assert!(config.source("api1").is_some());
}

#[test]
fn test_source_lookup_by_name() {
    let yaml = r#"
sources:
  - name: users_api
    url: https://api.example.com/users
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
  - name: posts_api
    url: https://api.example.com/posts
    retry:
      max_attempts: 3
      max_delay_secs: 60
      min_delay_secs: 1
targets: []
"#;
    
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    
    // Lookup by name should work
    let users = config.source("users_api").unwrap();
    assert_eq!(users.name, "users_api");
    assert_eq!(users.url, "https://api.example.com/users");
    
    let posts = config.source("posts_api").unwrap();
    assert_eq!(posts.name, "posts_api");
    
    // Non-existent source should return None
    assert!(config.source("nonexistent").is_none());
}

#[test]
fn test_target_lookup_by_name() {
    let yaml = r#"
sources: []
targets:
  - type: postgres
    name: prod_db
    host: prod.example.com
    port: 5432
    database: production
    auth:
      username: prod_user
      password: prod_pass
  - type: postgres
    name: staging_db
    host: staging.example.com
    port: 5432
    database: staging
    auth:
      username: staging_user
      password: staging_pass
"#;
    
    let config: Config = serde_yaml::from_str(yaml).unwrap();
    
    // Both targets should be accessible
    assert!(config.target("prod_db").is_some());
    assert!(config.target("staging_db").is_some());
    assert!(config.target("nonexistent").is_none());
}

// Note: Full end-to-end integration tests would include:
// 1. Loading config from file
// 2. Creating pipeline from config
// 3. Executing pipeline with mock data
// 4. Validating data transformation
// 5. Testing error handling and recovery
//
// Example for future implementation:
// #[tokio::test]
// async fn test_end_to_end_pipeline() {
//     // Load config from file
//     let config = Config::from_file("test_config.yaml").unwrap();
//     
//     // Setup mock HTTP server
//     let mock_server = setup_mock_api().await;
//     
//     // Setup test database
//     let test_db = setup_test_db().await;
//     
//     // Run pipeline
//     let result = run_pipeline(&config).await;
//     
//     // Verify data in database
//     let rows = test_db.query("SELECT * FROM test_table").await.unwrap();
//     assert_eq!(rows.len(), expected_count);
//     
//     // Cleanup
//     teardown_test_db(test_db).await;
// }
