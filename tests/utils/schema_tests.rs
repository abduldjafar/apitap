use apitap::utils::schema::{infer_schema_from_values, infer_schema_streaming};
use datafusion::arrow::datatypes::DataType;
use futures::stream;
use serde_json::{json, Value};
use std::pin::Pin;

#[test]
fn test_infer_schema_from_values_basic_types() {
    let values = vec![
        json!({
            "id": 1,
            "name": "Alice",
            "active": true,
            "score": 95.5
        }),
        json!({
            "id": 2,
            "name": "Bob",
            "active": false,
            "score": 87.3
        }),
    ];

    let schema = infer_schema_from_values(&values).unwrap();

    assert_eq!(schema.fields().len(), 4);
    
    // Check field names exist
    assert!(schema.field_with_name("id").is_ok());
    assert!(schema.field_with_name("name").is_ok());
    assert!(schema.field_with_name("active").is_ok());
    assert!(schema.field_with_name("score").is_ok());
}

#[test]
fn test_infer_schema_from_values_empty() {
    let values: Vec<Value> = vec![];
    let schema = infer_schema_from_values(&values).unwrap();
    
    assert_eq!(schema.fields().len(), 0);
}

#[test]
fn test_infer_schema_from_values_with_nulls() {
    let values = vec![
        json!({
            "id": 1,
            "name": "Alice",
            "email": null
        }),
        json!({
            "id": 2,
            "name": "Bob",
            "email": "bob@example.com"
        }),
    ];

    let schema = infer_schema_from_values(&values).unwrap();
    
    assert_eq!(schema.fields().len(), 3);
    
    // Email field should be nullable
    let email_field = schema.field_with_name("email").unwrap();
    assert!(email_field.is_nullable());
}

#[test]
fn test_infer_schema_from_values_nested_objects() {
    let values = vec![
        json!({
            "id": 1,
            "metadata": {
                "created": "2024-01-01",
                "updated": "2024-01-02"
            }
        }),
    ];

    let schema = infer_schema_from_values(&values).unwrap();
    
    // Nested objects should be present
    assert!(schema.field_with_name("id").is_ok());
    assert!(schema.field_with_name("metadata").is_ok());
}

#[test]
fn test_infer_schema_from_values_arrays() {
    let values = vec![
        json!({
            "id": 1,
            "tags": ["rust", "testing"]
        }),
    ];

    let schema = infer_schema_from_values(&values).unwrap();
    
    assert!(schema.field_with_name("id").is_ok());
    assert!(schema.field_with_name("tags").is_ok());
}

#[tokio::test]
async fn test_infer_schema_streaming_basic() {
    let values = vec![
        Ok(json!({"id": 1, "name": "Alice"})),
        Ok(json!({"id": 2, "name": "Bob"})),
        Ok(json!({"id": 3, "name": "Charlie"})),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    assert_eq!(schema.fields().len(), 2);
    assert!(schema.field_with_name("id").is_ok());
    assert!(schema.field_with_name("name").is_ok());
}

#[tokio::test]
async fn test_infer_schema_streaming_with_nulls() {
    let values = vec![
        Ok(json!({"id": 1, "name": "Alice", "email": null})),
        Ok(json!({"id": 2, "name": "Bob", "email": "bob@example.com"})),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    let email_field = schema.field_with_name("email").unwrap();
    assert!(email_field.is_nullable());
}

#[tokio::test]
async fn test_infer_schema_streaming_mixed_numeric_types() {
    let values = vec![
        Ok(json!({"id": 1, "value": 100})),
        Ok(json!({"id": 2, "value": 200.5})),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    // Mixed int/float should result in Float64
    let value_field = schema.field_with_name("value").unwrap();
    assert!(matches!(value_field.data_type(), DataType::Float64));
}

#[tokio::test]
async fn test_infer_schema_streaming_stops_at_min_samples() {
    // Create more than MIN_SAMPLES (100) items
    let mut values = vec![];
    for i in 0..150 {
        values.push(Ok(json!({"id": i, "name": format!("User{}", i)})));
    }
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    // Should still infer schema correctly without consuming all items
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    assert_eq!(schema.fields().len(), 2);
}

#[tokio::test]
async fn test_infer_schema_streaming_empty_stream() {
    let values: Vec<Result<Value, apitap::errors::ApitapError>> = vec![];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let result = infer_schema_streaming(boxed_stream).await;
    
    // Empty stream should return an error
    assert!(result.is_err());
}

#[tokio::test]
async fn test_infer_schema_streaming_only_non_objects() {
    let values = vec![
        Ok(json!("string1")),
        Ok(json!("string2")),
        Ok(json!(123)),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let result = infer_schema_streaming(boxed_stream).await;
    
    // Non-object values should result in error or empty schema
    assert!(result.is_err());
}

#[tokio::test]
async fn test_infer_schema_streaming_boolean_field() {
    let values = vec![
        Ok(json!({"id": 1, "active": true})),
        Ok(json!({"id": 2, "active": false})),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    let active_field = schema.field_with_name("active").unwrap();
    assert!(matches!(active_field.data_type(), DataType::Boolean));
}

#[tokio::test]
async fn test_infer_schema_streaming_nested_objects_as_strings() {
    let values = vec![
        Ok(json!({"id": 1, "data": {"nested": "value"}})),
        Ok(json!({"id": 2, "data": {"nested": "other"}})),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    // Nested objects should be treated as strings
    let data_field = schema.field_with_name("data").unwrap();
    assert!(matches!(data_field.data_type(), DataType::Utf8));
}

#[tokio::test]
async fn test_infer_schema_streaming_arrays_as_strings() {
    let values = vec![
        Ok(json!({"id": 1, "tags": ["tag1", "tag2"]})),
        Ok(json!({"id": 2, "tags": ["tag3"]})),
    ];
    
    let stream = stream::iter(values);
    let boxed_stream: Pin<Box<dyn futures::Stream<Item = Result<Value, apitap::errors::ApitapError>> + Send>> 
        = Box::pin(stream);
    
    let schema = infer_schema_streaming(boxed_stream).await.unwrap();
    
    // Arrays should be treated as strings
    let tags_field = schema.field_with_name("tags").unwrap();
    assert!(matches!(tags_field.data_type(), DataType::Utf8));
}
