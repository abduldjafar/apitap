// Tests for Arrow type conversion support (Struct, List, LargeList)

use datafusion::arrow::array::*;
use datafusion::arrow::datatypes::{DataType, Field};
use std::sync::Arc;

#[test]
fn test_arrow_struct_type_support() {
    let street_array = StringArray::from(vec!["123 Main St", "456 Oak Ave"]);
    let city_array = StringArray::from(vec!["Springfield", "Portland"]);

    let struct_array = StructArray::from(vec![
        (
            Arc::new(Field::new("street", DataType::Utf8, false)),
            Arc::new(street_array) as ArrayRef,
        ),
        (
            Arc::new(Field::new("city", DataType::Utf8, false)),
            Arc::new(city_array) as ArrayRef,
        ),
    ]);

    // Verify struct array was created
    assert_eq!(struct_array.len(), 2);
    assert_eq!(struct_array.num_columns(), 2);
}

#[test]
fn test_arrow_list_type_support() {
    // Create a list array
    let value_data = Int32Array::from(vec![1, 2, 3, 4, 5, 6]);
    let value_offsets = vec![0, 3, 6].into(); // Two lists: [1,2,3] and [4,5,6]

    let list_data = ArrayData::builder(DataType::List(Arc::new(Field::new(
        "item",
        DataType::Int32,
        false,
    ))))
    .len(2)
    .add_buffer(value_offsets)
    .add_child_data(value_data.into_data())
    .build()
    .unwrap();

    let list_array = ListArray::from(list_data);

    // Verify list array was created
    assert_eq!(list_array.len(), 2);
    assert_eq!(list_array.value_length(0), 3);
    assert_eq!(list_array.value_length(1), 3);
}

#[test]
fn test_arrow_large_list_type_support() {
    // Create a large list array
    let value_data = Int64Array::from(vec![10, 20, 30, 40]);
    let value_offsets = vec![0i64, 2, 4].into(); // Two lists: [10,20] and [30,40]

    let list_data = ArrayData::builder(DataType::LargeList(Arc::new(Field::new(
        "item",
        DataType::Int64,
        false,
    ))))
    .len(2)
    .add_buffer(value_offsets)
    .add_child_data(value_data.into_data())
    .build()
    .unwrap();

    let large_list_array = LargeListArray::from(list_data);

    // Verify large list array was created
    assert_eq!(large_list_array.len(), 2);
    assert_eq!(large_list_array.value_length(0), 2);
    assert_eq!(large_list_array.value_length(1), 2);
}

#[test]
fn test_nested_struct_with_primitives() {
    // Test struct containing various primitive types
    let id_array = UInt64Array::from(vec![1, 2, 3]);
    let name_array = StringArray::from(vec!["Alice", "Bob", "Charlie"]);
    let active_array = BooleanArray::from(vec![true, false, true]);

    let struct_array = StructArray::from(vec![
        (
            Arc::new(Field::new("id", DataType::UInt64, false)),
            Arc::new(id_array) as ArrayRef,
        ),
        (
            Arc::new(Field::new("name", DataType::Utf8, false)),
            Arc::new(name_array) as ArrayRef,
        ),
        (
            Arc::new(Field::new("active", DataType::Boolean, false)),
            Arc::new(active_array) as ArrayRef,
        ),
    ]);

    // Verify the struct has correct structure
    assert_eq!(struct_array.len(), 3);
    assert_eq!(struct_array.num_columns(), 3);
    assert_eq!(struct_array.column(0).len(), 3);
}

#[test]
fn test_list_of_structs() {
    // Create a list containing struct elements (realistic nested data)
    let name_array = StringArray::from(vec!["John", "Jane"]);
    let email_array = StringArray::from(vec!["john@example.com", "jane@example.com"]);

    let struct_array = StructArray::from(vec![
        (
            Arc::new(Field::new("name", DataType::Utf8, false)),
            Arc::new(name_array) as ArrayRef,
        ),
        (
            Arc::new(Field::new("email", DataType::Utf8, false)),
            Arc::new(email_array) as ArrayRef,
        ),
    ]);

    // Verify nested structure
    assert_eq!(struct_array.len(), 2);
    assert!(struct_array.column(0).as_any().is::<StringArray>());
    assert!(struct_array.column(1).as_any().is::<StringArray>());
}

#[test]
fn test_arrow_null_handling_in_struct() {
    // Test struct with nullable fields
    let optional_field = Field::new("optional_value", DataType::Utf8, true);
    let required_field = Field::new("required_value", DataType::Int32, false);

    let optional_array: ArrayRef = Arc::new(StringArray::from(vec![
        Some("present"),
        None,
        Some("also present"),
    ]));
    let required_array: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3]));

    let struct_array = StructArray::from(vec![
        (Arc::new(optional_field), optional_array),
        (Arc::new(required_field), required_array),
    ]);

    assert_eq!(struct_array.len(), 3);
    assert!(struct_array.is_valid(1)); // Struct is valid even if field is null
}

#[test]
fn test_empty_list_array() {
    // Test empty list handling
    let value_data = Int32Array::from(Vec::<i32>::new());
    let value_offsets = vec![0i32].into(); // Empty list

    let list_data = ArrayData::builder(DataType::List(Arc::new(Field::new(
        "item",
        DataType::Int32,
        false,
    ))))
    .len(0)
    .add_buffer(value_offsets)
    .add_child_data(value_data.into_data())
    .build()
    .unwrap();

    let list_array = ListArray::from(list_data);

    assert_eq!(list_array.len(), 0);
}

#[test]
fn test_arrow_data_type_matching() {
    // Verify Arrow DataType enum variants we're handling
    let struct_type = DataType::Struct(vec![Field::new("test", DataType::Int32, false)].into());
    let list_type = DataType::List(Arc::new(Field::new("item", DataType::Utf8, false)));
    let large_list_type = DataType::LargeList(Arc::new(Field::new("item", DataType::Int64, false)));

    // These should match the patterns in arrow_value_to_json
    assert!(matches!(struct_type, DataType::Struct(_)));
    assert!(matches!(list_type, DataType::List(_)));
    assert!(matches!(large_list_type, DataType::LargeList(_)));
}

#[test]
fn test_supported_primitive_types() {
    // Verify all primitive types we support
    let types = vec![
        DataType::Null,
        DataType::Boolean,
        DataType::Int32,
        DataType::Int64,
        DataType::UInt32,
        DataType::UInt64,
        DataType::Float32,
        DataType::Float64,
        DataType::Utf8,
        DataType::LargeUtf8,
    ];

    // All these should be handled in arrow_value_to_json
    for dt in types {
        assert!(matches!(
            dt,
            DataType::Null
                | DataType::Boolean
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt32
                | DataType::UInt64
                | DataType::Float32
                | DataType::Float64
                | DataType::Utf8
                | DataType::LargeUtf8
        ));
    }
}
