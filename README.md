# 🚰 Apitap

**Extract from REST APIs, transform with SQL, load to warehouses**

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![DataFusion](https://img.shields.io/badge/powered%20by-DataFusion-blue)](https://datafusion.apache.org/)
[![Learning Project](https://img.shields.io/badge/status-learning%20project-green)](https://github.com/yourusername/apitap)

[Quick Start](#-quick-start) • [Features](#-features) • [Examples](#-examples) • [Learning Journey](#-learning-journey) • [Documentation](#-documentation)

</div>

---

> 🌱 **Learning Project Notice**: This is a learning project built to enhance skills in Rust, systems programming, and data engineering. It's actively being developed and may not be production-ready. Feedback, contributions, and learning together are welcome!

---

## 🎯 What is Apitap?

Apitap is a lightweight ETL engine that I'm building to learn Rust, systems programming, and data engineering concepts. The goal is to make it simple to:

1. **📥 Extract** data from any HTTP/REST API
2. **🔄 Transform** using familiar SQL (powered by Apache DataFusion)
3. **📤 Load** into your favorite data warehouse

This project helps me dive deep into:
- Rust async programming with Tokio
- Apache Arrow & DataFusion internals
- Building streaming data pipelines
- Database connectors and protocols
- Systems programming concepts

```yaml
# Example: GitHub stars to PostgreSQL
source:
  http:
    url: https://api.github.com/repos/rust-lang/rust
    method: GET

transform:
  sql: |
    SELECT 
      name,
      stargazers_count as stars,
      CURRENT_TIMESTAMP as synced_at
    FROM api_response

destination:
  postgres:
    connection: postgresql://user:pass@localhost/mydb
    table: github_stars
```

## ✨ Features (Current & Planned)

- 🚀 **Fast & Lightweight** - Built in Rust, powered by Apache Arrow & DataFusion
- 🔌 **HTTP First** - Support for REST APIs, webhooks, pagination, auth
- 🧮 **SQL Transformations** - Use familiar SQL for complex data transformations
- 🎯 **Multiple Destinations** - PostgreSQL, BigQuery, ClickHouse (in progress)
- 📊 **Schema Inference** - Automatic schema detection from JSON responses (planned)
- 🔄 **Incremental Loads** - Support for cursor-based syncs (planned)
- ⚡ **Streaming** - Process large datasets efficiently (learning)

## 📚 Learning Journey

### What I'm Learning

**Rust & Systems Programming:**
- ✅ Ownership, borrowing, and lifetimes
- ✅ Async/await with Tokio runtime
- 🔄 Error handling patterns (Result, custom errors)
- 🔄 Generic programming and trait objects
- 📝 Building CLI tools with clap
- 📝 Testing strategies in Rust

**Data Engineering:**
- ✅ Apache Arrow memory format
- 🔄 DataFusion query engine integration
- 🔄 Streaming data processing
- 📝 Database connection pooling
- 📝 Schema evolution and type mapping
- 📝 Incremental data sync patterns

**Systems Concepts:**
- ✅ Channel-based communication (mpsc)
- 🔄 Backpressure handling
- 📝 Resource pooling
- 📝 Error recovery and retries
- 📝 Observability (metrics, logging)

**Legend:** ✅ Implemented • 🔄 In Progress • 📝 Planned

### Progress Tracker

- [ ] Basic HTTP client with reqwest
- [ ] JSON parsing and flattening
- [ ] DataFusion TableProvider implementation
- [ ] PostgreSQL writer with tokio-postgres
- [ ] Pagination strategies
- [ ] Authentication handlers (Bearer, OAuth2)
- [ ] BigQuery integration
- [ ] ClickHouse integration
- [ ] State management for incremental syncs
- [ ] Proper error handling and recovery
- [ ] Comprehensive test suite
- [ ] Performance benchmarking

## 🚀 Quick Start

### Installation

```bash
# Clone the repo
git clone https://github.com/yourusername/apitap.git
cd apitap

# Build
cargo build --release

# Run an example
cargo run --release -- run examples/simple.yaml
```

### Your First Pipeline

1. **Create a config file** (`pipeline.yaml`):

```yaml
source:
  http:
    url: https://jsonplaceholder.typicode.com/users
    method: GET

transform:
  sql: |
    SELECT 
      id,
      name,
      email,
      company.name as company_name
    FROM api_response

destination:
  postgres:
    connection: ${DATABASE_URL}
    table: users
    mode: replace
```

2. **Run it:**

```bash
cargo run --release -- run pipeline.yaml
```

## 💡 Examples

### Example 1: GitHub API to PostgreSQL

```yaml
source:
  http:
    url: https://api.github.com/repos/rust-lang/rust
    method: GET
    headers:
      Accept: application/vnd.github.v3+json

transform:
  sql: |
    SELECT 
      name,
      description,
      stargazers_count as stars,
      forks_count as forks,
      language,
      updated_at
    FROM api_response

destination:
  postgres:
    connection: postgresql://localhost/mydb
    table: github_repos
    mode: replace
```

### Example 2: REST API with SQL Filtering

```yaml
source:
  http:
    url: https://jsonplaceholder.typicode.com/posts
    method: GET

transform:
  sql: |
    SELECT 
      userId as user_id,
      id,
      title,
      LENGTH(body) as content_length
    FROM api_response
    WHERE userId <= 5
    ORDER BY id DESC

destination:
  postgres:
    connection: ${DATABASE_URL}
    table: posts
```

## 🎯 Current Capabilities

| Feature | Status | Notes |
|---------|--------|-------|
| HTTP GET requests | ✅ | Working |
| JSON parsing | ✅ | Nested objects supported |
| SQL transforms | ✅ | Via DataFusion |
| PostgreSQL output | ✅ | Basic insert/replace |
| BigQuery output | 🔄 | In progress |
| ClickHouse output | 📝 | Planned |
| Pagination | 📝 | Planned |
| Authentication | 📝 | Planned |
| Incremental sync | 📝 | Planned |
| Error recovery | 📝 | Planned |

✅ Working • 🔄 In Progress • 📝 Planned

## 🏗️ Architecture (Current)

```
┌─────────────┐      ┌──────────────┐      ┌─────────────┐
│             │      │              │      │             │
│  HTTP GET   │─────▶│  DataFusion  │─────▶│  PostgreSQL │
│  (reqwest)  │      │  SQL Query   │      │  (tokio-pg) │
│             │      │              │      │             │
└─────────────┘      └──────────────┘      └─────────────┘
   Extract              Transform              Load
```

**Tech Stack I'm Learning:**
- `tokio` - Async runtime
- `reqwest` - HTTP client
- `datafusion` - SQL query engine
- `arrow` - Columnar data format
- `tokio-postgres` - PostgreSQL async driver
- `serde_json` - JSON parsing
- `clap` - CLI argument parsing

## 🤝 Contributing & Learning Together

I'm learning in public! If you're also learning Rust or data engineering, feel free to:

- 🐛 Report bugs or issues you find
- 💡 Suggest improvements or features
- 📖 Share learning resources
- 🔧 Submit PRs (with explanations of what you learned!)
- 💬 Discuss approaches in GitHub Issues

```bash
# Get started
git clone https://github.com/yourusername/apitap.git
cd apitap
cargo build

# Run tests
cargo test

# Try an example
cargo run -- run examples/github-stars.yaml
```

## 📖 Documentation & Resources

### Learning Resources I'm Using:
- [The Rust Book](https://doc.rust-lang.org/book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [DataFusion Documentation](https://datafusion.apache.org/)
- [Arrow Format Specification](https://arrow.apache.org/docs/format/Columnar.html)
- [Designing Data-Intensive Applications](https://dataintensive.net/)

### Project Documentation:
- [Architecture Notes](docs/architecture.md) - How things work
- [Development Log](docs/dev-log.md) - My learning notes
- [API Reference](docs/api.md) - Code documentation

## 🛣️ Learning Roadmap

**Phase 1: Core Pipeline (Current)**
- [ ] HTTP source basics
- [ ] DataFusion integration
- [ ] PostgreSQL writer
- [ ] Error handling patterns
- [ ] Comprehensive tests

**Phase 2: Production Features**
- [ ] Authentication (Bearer, OAuth2)
- [ ] Pagination strategies
- [ ] State management
- [ ] Retry logic with backoff
- [ ] Connection pooling

**Phase 3: Advanced**
- [ ] Multiple destinations
- [ ] Schema evolution
- [ ] Incremental syncs
- [ ] Performance optimization
- [ ] Observability (metrics, traces)

**Phase 4: Stretch Goals**
- [ ] GraphQL support
- [ ] dbt integration
- [ ] Web UI for monitoring
- [ ] Distributed mode

## 💬 Connect

- 💭 [GitHub Discussions](https://github.com/yourusername/apitap/discussions) - Ask questions, share ideas
- 🐛 [Issues](https://github.com/yourusername/apitap/issues) - Bug reports, feature requests
- 📝 [Dev Log](docs/dev-log.md) - Follow my learning journey

## 📄 License

MIT License - See [LICENSE](LICENSE) for details.

## 🙏 Acknowledgments

Huge thanks to the amazing Rust and data engineering communities, and these incredible open-source projects:

- [Apache Arrow](https://arrow.apache.org/) & [DataFusion](https://datafusion.apache.org/) - For making this possible
- [Tokio](https://tokio.rs/) - Async runtime
- [The Rust Community](https://www.rust-lang.org/community) - For patience with beginners

Special shoutout to:
- [datafusion-contrib](https://github.com/datafusion-contrib) repositories for examples
- Rust async book authors
- Everyone who answers questions on Discord/Reddit

---

<div align="center">

**Learning Rust? Data Engineering? Join me!**

[⭐ Star this repo](https://github.com/yourusername/apitap) • [📖 Read the Dev Log](docs/dev-log.md) • [💬 Start a Discussion](https://github.com/yourusername/apitap/discussions)

*Built with ❤️ while learning Rust, systems programming, and data engineering*


**Key changes:**

1. **🌱 Learning Project Badge & Notice** at the top
2. **📚 Learning Journey Section** showing what you're learning (Rust, systems programming, data engineering)
3. **Progress Tracker** with checkboxes (✅ 🔄 📝)
4. **Learning Resources** section with books/docs you're using
5. **Honest Status Table** showing what's working vs. planned
6. **Learning Roadmap** in phases
7. **"Learning Together"** contribution section - inviting others to learn with you
8. **Dev Log** references - encouraging public learning
9. **Humble tone** while still being professional
10. **Acknowledgments** to the community

This sets proper expectations while being inspiring and inviting others to join your learning journey! 🚀
