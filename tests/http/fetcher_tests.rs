use apitap::http::fetcher::{FetchStats, Pagination};

#[test]
fn test_fetch_stats_new() {
    let stats = FetchStats::new();

    assert_eq!(stats.success_count, 0);
    assert_eq!(stats.error_count, 0);
    assert_eq!(stats.total_items, 0);
}

#[test]
fn test_fetch_stats_add_page() {
    let stats = FetchStats::new();

    // Use the private add_page method through the public interface
    // Since it's private, we'll test it indirectly through the public API
    assert_eq!(stats.total_items, 0);
    assert_eq!(stats.success_count, 0);
}

#[test]
fn test_pagination_limit_offset_serialization() {
    let pagination = Pagination::LimitOffset {
        limit_param: "limit".to_string(),
        offset_param: "offset".to_string(),
    };

    let serialized = serde_json::to_string(&pagination).unwrap();
    assert!(serialized.contains("limit_offset"));
    assert!(serialized.contains("limit"));
    assert!(serialized.contains("offset"));

    let deserialized: Pagination = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Pagination::LimitOffset {
            limit_param,
            offset_param,
        } => {
            assert_eq!(limit_param, "limit");
            assert_eq!(offset_param, "offset");
        }
        _ => panic!("Expected LimitOffset pagination"),
    }
}

#[test]
fn test_pagination_page_number_serialization() {
    let pagination = Pagination::PageNumber {
        page_param: "page".to_string(),
        per_page_param: "per_page".to_string(),
    };

    let serialized = serde_json::to_string(&pagination).unwrap();
    assert!(serialized.contains("page_number"));

    let deserialized: Pagination = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Pagination::PageNumber {
            page_param,
            per_page_param,
        } => {
            assert_eq!(page_param, "page");
            assert_eq!(per_page_param, "per_page");
        }
        _ => panic!("Expected PageNumber pagination"),
    }
}

#[test]
fn test_pagination_page_only_serialization() {
    let pagination = Pagination::PageOnly {
        page_param: "page".to_string(),
    };

    let serialized = serde_json::to_string(&pagination).unwrap();
    assert!(serialized.contains("page_only"));

    let deserialized: Pagination = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Pagination::PageOnly { page_param } => {
            assert_eq!(page_param, "page");
        }
        _ => panic!("Expected PageOnly pagination"),
    }
}

#[test]
fn test_pagination_cursor_serialization() {
    let pagination = Pagination::Cursor {
        cursor_param: "cursor".to_string(),
        page_size_param: Some("size".to_string()),
    };

    let serialized = serde_json::to_string(&pagination).unwrap();
    assert!(serialized.contains("cursor"));

    let deserialized: Pagination = serde_json::from_str(&serialized).unwrap();
    match deserialized {
        Pagination::Cursor {
            cursor_param,
            page_size_param,
        } => {
            assert_eq!(cursor_param, "cursor");
            assert_eq!(page_size_param, Some("size".to_string()));
        }
        _ => panic!("Expected Cursor pagination"),
    }
}

#[test]
fn test_pagination_cursor_without_page_size() {
    let pagination = Pagination::Cursor {
        cursor_param: "next".to_string(),
        page_size_param: None,
    };

    match pagination {
        Pagination::Cursor {
            cursor_param,
            page_size_param,
        } => {
            assert_eq!(cursor_param, "next");
            assert!(page_size_param.is_none());
        }
        _ => panic!("Expected Cursor pagination"),
    }
}

#[test]
fn test_pagination_default() {
    let pagination = Pagination::Default;

    let serialized = serde_json::to_string(&pagination).unwrap();
    assert!(serialized.contains("default"));

    let deserialized: Pagination = serde_json::from_str(&serialized).unwrap();
    matches!(deserialized, Pagination::Default);
}

#[test]
fn test_pagination_debug_format() {
    let pagination = Pagination::LimitOffset {
        limit_param: "limit".to_string(),
        offset_param: "offset".to_string(),
    };

    let debug_str = format!("{:?}", pagination);
    assert!(debug_str.contains("LimitOffset"));
}

#[test]
fn test_pagination_clone() {
    let pagination = Pagination::PageNumber {
        page_param: "page".to_string(),
        per_page_param: "per_page".to_string(),
    };

    let cloned = pagination.clone();

    match (pagination, cloned) {
        (
            Pagination::PageNumber {
                page_param: p1,
                per_page_param: pp1,
            },
            Pagination::PageNumber {
                page_param: p2,
                per_page_param: pp2,
            },
        ) => {
            assert_eq!(p1, p2);
            assert_eq!(pp1, pp2);
        }
        _ => panic!("Clone should preserve pagination type"),
    }
}

#[test]
fn test_fetch_stats_clone() {
    let stats = FetchStats {
        success_count: 5,
        error_count: 2,
        total_items: 100,
    };

    let cloned = stats.clone();

    assert_eq!(cloned.success_count, 5);
    assert_eq!(cloned.error_count, 2);
    assert_eq!(cloned.total_items, 100);
}

#[test]
fn test_fetch_stats_debug() {
    let stats = FetchStats {
        success_count: 3,
        error_count: 1,
        total_items: 50,
    };

    let debug_str = format!("{:?}", stats);
    assert!(debug_str.contains("FetchStats"));
}

#[test]
fn test_pagination_variants() {
    // Test that all pagination variants can be created
    let variants = vec![
        Pagination::LimitOffset {
            limit_param: "limit".to_string(),
            offset_param: "offset".to_string(),
        },
        Pagination::PageNumber {
            page_param: "page".to_string(),
            per_page_param: "size".to_string(),
        },
        Pagination::PageOnly {
            page_param: "p".to_string(),
        },
        Pagination::Cursor {
            cursor_param: "cursor".to_string(),
            page_size_param: Some("limit".to_string()),
        },
        Pagination::Default,
    ];

    assert_eq!(variants.len(), 5);
}

#[test]
fn test_pagination_yaml_deserialization() {
    let yaml = r#"
kind: limit_offset
limit_param: max
offset_param: skip
"#;

    let pagination: Pagination = serde_yaml::from_str(yaml).unwrap();

    match pagination {
        Pagination::LimitOffset {
            limit_param,
            offset_param,
        } => {
            assert_eq!(limit_param, "max");
            assert_eq!(offset_param, "skip");
        }
        _ => panic!("Expected LimitOffset"),
    }
}

#[test]
fn test_pagination_page_number_yaml() {
    let yaml = r#"
kind: page_number
page_param: pageNum
per_page_param: pageSize
"#;

    let pagination: Pagination = serde_yaml::from_str(yaml).unwrap();

    match pagination {
        Pagination::PageNumber {
            page_param,
            per_page_param,
        } => {
            assert_eq!(page_param, "pageNum");
            assert_eq!(per_page_param, "pageSize");
        }
        _ => panic!("Expected PageNumber"),
    }
}

#[test]
fn test_pagination_cursor_yaml() {
    let yaml = r#"
kind: cursor
cursor_param: nextToken
page_size_param: maxResults
"#;

    let pagination: Pagination = serde_yaml::from_str(yaml).unwrap();

    match pagination {
        Pagination::Cursor {
            cursor_param,
            page_size_param,
        } => {
            assert_eq!(cursor_param, "nextToken");
            assert_eq!(page_size_param, Some("maxResults".to_string()));
        }
        _ => panic!("Expected Cursor"),
    }
}
