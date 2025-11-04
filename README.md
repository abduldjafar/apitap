# 🚰 Apitap

**Extract from REST APIs, transform with SQL, load to warehouses**
*HTTP-to-warehouse ETL powered by Apache DataFusion*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
[![DataFusion](https://img.shields.io/badge/powered%20by-DataFusion-blue)](https://datafusion.apache.org/)

**Quick links:** [Quick Start](#-quick-start) • [Features](#-features) • [Examples](#-examples) • [Architecture](#-architecture) • [Roadmap](#%EF%B8%8F-roadmap)

---

> 🌱 **Learning Project Notice**
> This is an active learning project exploring Rust, DataFusion, and ETL design. Expect breaking changes and rough edges. Feedback and PRs welcome!

---

## 🎯 What is Apitap?

Apitap is a lightweight ETL engine that:

1. **Extracts** JSON from HTTP/REST APIs (with pagination)
2. **Transforms** it using SQL (Apache DataFusion)
3. **Loads** it into data stores (PostgreSQL today; others to follow)

### Why this approach?

* Author transformations as **SQL modules** (Minijinja templates)
* Declare inputs/outputs in the SQL via tiny helpers:

  * `{{ sink(name="postgres_sink") }}`
  * `select * from {{ use_source("json_place_holder") }};`
* Keep runtime behavior in **YAML config** (URLs, pagination, destinations)

---

## ✨ Features

**Working now**

* ✅ Minijinja-based SQL modules with `sink()` and `use_source()`
* ✅ Loader for module trees (`--modules` folder)
* ✅ Capture of sink & source names at render time
* ✅ HTTP client (reqwest) + pagination driver (`LimitOffset`, `PageNumber`, `PageOnly`, `Cursor`)
* ✅ DataFusion-backed SQL execution
* ✅ PostgreSQL writer (auto-create, merge/upsert by PK)
* ✅ Writer factory (add new sinks without changing `main`)
* ✅ CLI runner (`apitap-run`) with `--modules` and `--yaml-config`

**In progress / planned**

* 🔄 ClickHouse writer
* 🔄 BigQuery writer
* 🔄 Incremental sync state
* 🔄 Auth strategies (Bearer/OAuth2), retries/backoff
* 🔄 Schema inference + evolution
* 🔄 Observability (metrics/logging), benchmarks

Legend: ✅ Working • 🔄 In Progress • 📝 Planned

---

## 🚀 Quick Start

### 1) Project layout

```
src/
  lib.rs
  config/
    templating.rs         # build_env_with_captures, list_sql_templates, render_one
    mod.rs
  http/
    fetcher.rs            # PaginatedFetcher + DataFusionPageWriter integration
  pipeline/
    sink.rs               # MakeWriter + WriterOpts (factory to DataWriter)
    run.rs                # run_fetch (pagination -> page writer -> sink)
    mod.rs
  writer/
    postgres.rs           # PostgresWriter implementing DataWriter
    mod.rs
  cmd/
    runner.rs             # run_pipeline(root, cfg_path)
bin/
  apitap-run.rs           # small CLI that calls cmd::runner
```

### 2) Build

```bash
cargo build --release
```

### 3) Prepare modules & config

```
pipelines/
  placeholder/
    post.sql
pipelines.yaml
```

**`pipelines/placeholder/post.sql`**

```sql
{{ sink(name="postgres_sink") }}

select *
from {{ use_source("json_place_holder") }};
```

**`pipelines.yaml` (shape example; adapt to your schema)**

```yaml
sources:
  - name: json_place_holder
    url: https://jsonplaceholder.typicode.com/posts
    table_destination_name: posts
    pagination:
      kind: limit_offset
      limit_param: _limit
      offset_param: _start

targets:
  - name: postgres_sink
    type: postgres
    auth:
      # You can provide credentials directly:
      # username: postgres
      # password: postgres
      # Or reference environment variables (recommended):
      username_env: POSTGRES_USER
      password_env: POSTGRES_PASSWORD
    host: localhost
    database: postgres
```

### 4) Run

The runner:

* discovers `.sql` under `--modules`
* renders each with Minijinja (captures `sink` + `source`)
* resolves the source/target from YAML
* replaces `{{ use_source("X") }}` with the configured destination table name
* fetches via HTTP (using the configured pagination)
* runs the DataFusion SQL
* writes into the sink (Postgres merge/upsert by `id`)

---

## 🧪 Examples

### Minimal module

```sql
{{ sink(name="postgres_sink") }}

select * from {{ use_source("json_place_holder") }};
```

### Multiple helpers (add your own!)

You can register more Minijinja helpers (e.g., `use_schema("...")`, `mode("append")`) the same way `sink`/`use_source` are wired today.

---

## 🏗️ Architecture

```
              ┌──────────────┐
              │  CLI (clap)  │  apitap-run --modules DIR --yaml-config FILE
              └───────┬──────┘
                      │
              ┌───────▼────────────────────┐
              │ cmd::runner::run_pipeline  │
              └───────┬────────────────────┘
                      │
     ┌────────────────┴──────────────┐
     │ config::templating            │  build_env_with_captures()
     │  • list_sql_templates         │  register sink()/use_source()
     │  • render_one                 │  capture sink/source
     └────────────────┬──────────────┘
                      │
               ┌──────▼──────────┐
               │ YAML config      │  sources: URL + pagination + table_destination_name
               │ (load_config_…)  │  targets: named sinks (e.g., postgres_sink)
               └──────┬──────────┘
                      │
       ┌──────────────▼──────────────┐
       │ HTTP + Pagination            │  PaginatedFetcher
       │  • reqwest client            │  LimitOffset / PageNumber / PageOnly / Cursor
       └──────────────┬──────────────┘
                      │
            ┌─────────▼─────────┐
            │ DataFusion SQL     │  DataFusionPageWriter executes SQL
            └─────────┬─────────┘
                      │
     ┌────────────────▼────────────────┐
     │ pipeline::sink::MakeWriter      │  TargetConn -> Arc<dyn DataWriter>
     │  • PostgresWriter (upsert)      │  (factory; extend for CH/BQ)
     └─────────────────────────────────┘
```

* **Templating:** Minijinja captures `sink`/`source` from the SQL module itself.
* **Runner:** walks all `.sql`, renders, resolves config, executes.
* **Fetcher:** generic pagination → stream pages → DataFusion plan → writer.
* **Writer factory:** add new sinks in one place (no big `match` in main).

---

## ⚙️ CLI

```
apitap --modules <DIR> --yaml-config <FILE>
```

* `--modules, -m` (default: `pipelines`) — Folder of SQL templates
* `--yaml-config, -y` (default: `pipelines.yaml`) — Pipeline config file

Help text:

> Extract from REST APIs, transform with SQL, load to warehouses.
> HTTP-to-warehouse ETL powered by DataFusion.

---

## 🛣️ Roadmap

**Core**

* [x] Minijinja modules + capture
* [x] Pagination driver (LO/PN/PO/Cursor)
* [x] DataFusion execution
* [x] Postgres writer (merge by `id`)
* [x] Writer factory (no main-branching)
* [x] CLI with `--modules` / `--yaml-config`

**Next**

* [ ] ClickHouse writer
* [ ] BigQuery writer
* [ ] Auth, retries/backoff
* [ ] State for incremental loads
* [ ] Schema inference / evolution
* [ ] Logging/metrics + perf tuning
* [ ] Tests and CI

---

## 📚 Learning Notes

* Rust async with Tokio
* Traits + trait objects (`Arc<dyn DataWriter>`)
* DataFusion logical plans
* Backpressure and pagination
* Clear module boundaries (`config/`, `pipeline/`, `cmd/`)

---

## 🤝 Contributing

New to Rust/data? Perfect—this is a learning repo.
PRs, ideas, docs, and questions are welcome.

```bash
git clone https://github.com/yourusername/apitap.git
cd apitap
cargo build
cargo test
```

Run a pipeline:

```bash
cargo run -- --modules ./pipelines --yaml-config ./pipelines.yaml
```

Credential management
---------------------

For security it's recommended to avoid hardcoding credentials in YAML. Instead, set environment variables and reference them in your pipeline config:

```yaml
targets:
  - name: postgres_sink
    type: postgres
    auth:
      username_env: POSTGRES_USER
      password_env: POSTGRES_PASSWORD
    host: localhost
    database: postgres
```

The runner will load a local `.env` file if present (via `dotenvy`) and will validate that referenced environment variables exist and are non-empty at startup. If credentials are missing, apitap will fail with a configuration error explaining what's missing.

---

## 📄 License

MIT — see [LICENSE](LICENSE).

---

## 🙏 Acknowledgments

* [Apache Arrow](https://arrow.apache.org/) & [DataFusion](https://datafusion.apache.org/)
* [Tokio](https://tokio.rs/) and the Rust community ❤️
