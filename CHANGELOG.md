# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Production readiness report and refactoring plan
- Comprehensive CONTRIBUTING.md guide
- SECURITY.md with vulnerability reporting process
- CODE_OF_CONDUCT.md for community guidelines
- .env.example template for environment variables
- Complete Cargo.toml metadata for crates.io compatibility

### Changed
- Fixed Cargo.toml edition from invalid 2024 to 2021
- Improved code organization and module structure

### Fixed
- Cargo.toml edition compatibility

## [0.1.0] - 2024

### Added
- Initial release of ApiTap
- HTTP-to-warehouse ETL engine powered by Apache DataFusion
- SQL transformation with Minijinja templating
- Smart pagination support:
  - LimitOffset pagination
  - PageNumber pagination
  - PageOnly pagination
  - Cursor-based pagination
- PostgreSQL writer with MERGE/upsert support
- Automatic retry with exponential backoff
- Structured logging with tracing (JSON and human-readable)
- CLI with comprehensive flags
- Configuration via YAML files
- Module loader for SQL files
- Template engine with sink() and use_source() helpers
- Writer factory pattern for extensibility
- Performance optimizations:
  - Optimized batch sizes (5000 rows per batch)
  - Lock-free atomic counters
  - Efficient async I/O with large channel buffers
- Comprehensive documentation:
  - Detailed README with examples
  - Flamegraph analysis report
  - Optimization reports
  - Architecture documentation

### Performance
- 2-5x faster database writes through optimization
- Competitive with industry-standard ETL tools
- Low overhead JSON streaming
- Efficient concurrent HTTP requests

### Documentation
- Comprehensive README with usage examples
- Architecture diagrams and explanations
- Configuration reference
- Performance tuning guide
- Quick start guide
- Roadmap and feature tracking

### Infrastructure
- Basic GitHub Actions CI/CD workflow
- MIT License
- .gitignore configuration
- Example configurations and SQL modules

## [0.0.1] - Initial Development

### Added
- Project structure setup
- Core architecture design
- Basic HTTP client
- DataFusion integration
- PostgreSQL writer prototype

---

## Release Types

- **Major (x.0.0)**: Breaking API changes
- **Minor (0.x.0)**: New features, backward compatible
- **Patch (0.0.x)**: Bug fixes and minor improvements

## Categories

- **Added**: New features
- **Changed**: Changes in existing functionality
- **Deprecated**: Soon-to-be removed features
- **Removed**: Removed features
- **Fixed**: Bug fixes
- **Security**: Security fixes and improvements
- **Performance**: Performance improvements

---

[Unreleased]: https://github.com/abduldjafar/apitap/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/abduldjafar/apitap/releases/tag/v0.1.0
[0.0.1]: https://github.com/abduldjafar/apitap/releases/tag/v0.0.1
