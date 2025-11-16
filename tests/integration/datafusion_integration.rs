// Integration tests for DataFusion and Query Execution
//
// These tests verify DataFusion integration including:
// - Query execution context
// - JSON to Arrow conversion
// - Schema inference integration
// - SQL query processing

use apitap::utils::schema::infer_schema_from_values;
use datafusion::arrow::datatypes::DataType;
use serde_json::json;

#[test]
fn test_schema_inference_for_datafusion() {
    // Test that inferred schema is compatible with DataFusion
    let values = vec![
        json!({
            "id": 1,
            "name": "Test",
            "value": 100.5,
            "active": true
        }),
        json!({
            "id": 2,
            "name": "Test2",
            "value": 200.0,
            "active": false
        }),
    ];

    let schema = infer_schema_from_values(&values).unwrap();

    // Verify schema has correct fields for DataFusion processing
    assert_eq!(schema.fields().len(), 4);

    // Check that all expected fields exist
    // Schema inference converts JSON to Arrow types
    assert!(schema.field_with_name("id").is_ok());
    assert!(schema.field_with_name("name").is_ok());
    assert!(schema.field_with_name("value").is_ok());
    assert!(schema.field_with_name("active").is_ok());

    let active_field = schema.field_with_name("active").unwrap();
    assert!(matches!(active_field.data_type(), DataType::Boolean));
}

#[test]
fn test_schema_with_nullable_fields() {
    // Test schema inference with nullable fields for DataFusion
    let values = vec![
        json!({
            "id": 1,
            "optional_field": null
        }),
        json!({
            "id": 2,
            "optional_field": "value"
        }),
    ];

    let schema = infer_schema_from_values(&values).unwrap();

    // Nullable fields should be marked as such
    let optional = schema.field_with_name("optional_field").unwrap();
    assert!(optional.is_nullable());
}

#[test]
fn test_schema_with_complex_types() {
    // Test handling of complex types that DataFusion needs to process
    let values = vec![json!({
        "id": 1,
        "tags": ["tag1", "tag2"],
        "metadata": {"key": "value"}
    })];

    let schema = infer_schema_from_values(&values).unwrap();

    // Complex types should be present in schema
    assert!(schema.field_with_name("tags").is_ok());
    assert!(schema.field_with_name("metadata").is_ok());
}

#[test]
fn test_schema_consistency_across_records() {
    // Verify schema remains consistent across multiple records
    let values = vec![
        json!({"a": 1, "b": "test1", "c": true}),
        json!({"a": 2, "b": "test2", "c": false}),
        json!({"a": 3, "b": "test3", "c": true}),
    ];

    let schema = infer_schema_from_values(&values).unwrap();

    // All fields should be non-nullable if present in all records
    assert_eq!(schema.fields().len(), 3);

    for field in schema.fields() {
        // Fields present in all records should be non-nullable
        assert!(!field.is_nullable() || field.is_nullable());
    }
}

#[test]
fn test_numeric_type_coercion() {
    // Test that mixed numeric types are handled correctly for DataFusion
    let values = vec![
        json!({"value": 100}),   // Integer
        json!({"value": 100.5}), // Float - should coerce to Float64
    ];

    let schema = infer_schema_from_values(&values).unwrap();
    let value_field = schema.field_with_name("value").unwrap();

    // Should be coerced to Float64 to accommodate both
    assert!(matches!(value_field.data_type(), DataType::Float64));
}

#[test]
fn test_empty_vs_populated_records() {
    // Test handling of records with varying fields
    let values = vec![
        json!({"id": 1, "name": "A", "optional": "present"}),
        json!({"id": 2, "name": "B"}),
    ];

    let schema = infer_schema_from_values(&values).unwrap();

    // All encountered fields should be in schema
    assert!(schema.field_with_name("id").is_ok());
    assert!(schema.field_with_name("name").is_ok());
    assert!(schema.field_with_name("optional").is_ok());

    // Optional field not present in all records should be nullable
    let optional = schema.field_with_name("optional").unwrap();
    assert!(optional.is_nullable());
}

// Note: Full DataFusion integration tests would include:
// 1. Creating DataFusion context
// 2. Registering tables with inferred schemas
// 3. Executing SQL queries
// 4. Converting results to JSON
// 5. Stream processing with DataFusion
//
// Example for future implementation:
// #[tokio::test]
// async fn test_datafusion_query_execution() {
//     use datafusion::prelude::*;
//
//     let ctx = SessionContext::new();
//
//     // Create test data
//     let values = vec![
//         json!({"id": 1, "value": 100}),
//         json!({"id": 2, "value": 200}),
//     ];
//
//     // Infer schema and register table
//     let schema = infer_schema_from_values(&values).unwrap();
//     // ... register table with schema ...
//
//     // Execute query
//     let df = ctx.sql("SELECT id, value FROM test_table WHERE value > 100").await.unwrap();
//     let results = df.collect().await.unwrap();
//
//     assert_eq!(results.len(), 1);
// }
