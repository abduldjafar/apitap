// Integration tests for apitap
//
// This test suite is organized into modules for better maintainability:
// - config: Tests for configuration and templating
// - errors: Tests for error handling and error types
// - utils: Tests for utility functions (schema inference, streaming)
// - pipeline: Tests for pipeline configuration and management
// - http: Tests for HTTP fetcher and pagination
// - writer: Tests for data writer and write modes

mod config;
mod errors;
mod http;
mod pipeline;
mod utils;
mod writer;
