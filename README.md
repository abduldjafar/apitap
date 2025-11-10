# 🚰 Apitap

**Stream JSON from REST APIs, transform with SQL, load into your warehouse**  
*Tiny HTTP-to-warehouse ETL engine powered by Apache DataFusion & Rust*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
[![DataFusion](https://img.shields.io/badge/powered%20by-DataFusion-blue)](https://datafusion.apache.org/)

**Quick links:**  
[What is Apitap?](#-what-is-apitap) • [Features](#-features) • [Quick Start](#-quick-start) • [Examples](#-examples) • [Architecture](#-architecture) • [Roadmap](#%EF%B8%8F-roadmap)

---

## 🎯 What is Apitap?

Apitap is a lightweight ETL engine that:

1. **Extracts** JSON from HTTP/REST APIs (with pagination)
2. **Transforms** it using **SQL** (Apache DataFusion)
3. **Loads** it into data stores (PostgreSQL today; ClickHouse/BigQuery soon)

You describe:

- **What to do** in SQL modules (with tiny Minijinja helpers), and  
- **Where to get data / where to put it** in a YAML config.

Apitap does the boring bits: pagination, retries, streaming JSON into DataFusion, and upserting into your database.

### Who is this for?

- You like **Rust** and **SQL** and want a simple HTTP-to-warehouse tool  
- You have a few APIs (analytics, SaaS tools, internal services) and don’t want to run a huge ETL platform  
- You’d rather keep transformations as **SQL files in git** than scattered across app code

It’s great for small/medium data stacks, side projects, and learning DataFusion.

---

## ⚠️ Status

> **Early stage / learning project**  
> This repo explores Rust, DataFusion, and ETL design. Expect breaking changes, rough edges, and sharp corners. Feedback and PRs are very welcome.

---

## ✨ Features

### Working now

- 🧩 **SQL modules with Minijinja**  
  - `{{ sink(name="postgres_sink") }}` declares a target  
  - `{{ use_source("json_place_holder") }}` binds a source table  
- 📁 **Module loader** for a whole `--modules` folder of `.sql` files
- 🎭 **Templating env** that captures sinks & sources at render time
- 🌐 **HTTP client + pagination driver**
  - ✅ **LimitOffset** (e.g. `_limit` + `_start` style)
  - 📝 Other modes (PageNumber, PageOnly, Cursor) **planned**
- 🧠 **DataFusion-backed SQL execution**
- 🐘 **PostgreSQL writer**
  - Auto-create tables
  - Merge/upsert by primary key
- 🏭 **Writer factory** to add new sinks without touching `main`
- 🖥️ **CLI runner** (`apitap-run`) with:
  - `--modules` (SQL folder)
  - `--yaml-config` (pipeline config)

### In progress / planned

- 🔄 Pagination modes: PageNumber, PageOnly, Cursor
- 🔄 ClickHouse writer
- 🔄 BigQuery writer
- 🔄 Incremental sync state
- 🔄 Auth strategies (Bearer/OAuth2), retries/backoff tuning
- 🔄 Schema inference + evolution
- 🔄 Observability (metrics/logging), benchmarks

Legend: ✅ Working • 🔄 In Progress • 📝 Planned

---

## 🤔 Why Apitap vs other tools?

There are great tools out there (Airbyte, Singer/Meltano, dlt, Benthos, etc.). Apitap intentionally stays small:

- **Small footprint** – a single Rust binary, no JVM, no orchestrator required  
- **SQL-first transforms** – everything goes through Apache DataFusion SQL  
- **Config in git** – SQL modules + YAML config, no DB-backed GUI  
- **Focused on HTTP JSON → DB** – not trying to be “connectors for everything”

Use Apitap if:

- You want something you can **read end-to-end** in one repo
- You like the idea of **“SQL as pipeline spec”** (with a bit of templating)
- You’re curious about **DataFusion** and want real-world examples

---

## 🚀 Quick Start

### 1) Project layout

```text
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
````

### 2) Build

```bash
cargo build --release
```

### 3) Prepare modules & config

```text
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

**`pipelines.yaml`** (shape example; adapt to your schema)

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

```bash
cargo run --bin apitap-run -- \
  --modules ./pipelines \
  --yaml-config ./pipelines.yaml
```

What happens:

1. The runner discovers `.sql` under `--modules`
2. It renders them with Minijinja, capturing `sink()` and `use_source()`
3. It resolves sources/targets from YAML
4. It replaces `{{ use_source("X") }}` with the configured table name
5. It fetches data via HTTP **using LimitOffset pagination**
6. It runs the DataFusion SQL
7. It writes into the sink (Postgres merge/upsert by `id`)

---

## 🧪 Examples

### Minimal module

```sql
{{ sink(name="postgres_sink") }}

select * from {{ use_source("json_place_holder") }};
```

### Extend with your own helpers

You can register more Minijinja helpers (e.g. `use_schema("...")`, `mode("append")`) in the same place `sink`/`use_source` are wired today.

For example:

```sql
{{ sink(name="postgres_sink") }}
{{ mode("append") }}

select
  id,
  title,
  body,
  now() as loaded_at
from {{ use_source("json_place_holder") }};
```

---

## 🏗️ Architecture

```text
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
       │ HTTP + Pagination            │  
       │  • reqwest client            │  
       │  • **LimitOffset** driver    │  (PageNumber/PageOnly/Cursor planned)
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

* **Templating:** Minijinja captures sink/source info from the SQL itself
* **Runner:** walks all `.sql`, renders, resolves config, executes
* **Fetcher:** LimitOffset pagination → page writer → DataFusion → sink
* **Writer factory:** add new sinks in one place

---

## ⚙️ CLI

```text
apitap-run --modules <DIR> --yaml-config <FILE>
```

* `--modules, -m` (default: `pipelines`) — Folder of SQL templates
* `--yaml-config, -y` (default: `pipelines.yaml`) — Pipeline config file

---

## 🔐 Credential management

For security it’s recommended to avoid hardcoding credentials in YAML. Instead, set environment variables and reference them in your pipeline config:

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

The runner will load a local `.env` file if present (via `dotenvy`) and will validate that referenced environment variables exist and are non-empty at startup. If credentials are missing, Apitap will fail with a configuration error explaining what's missing.

---

## 🛣️ Roadmap

**Core**

* [x] Minijinja modules + capture
* [x] LimitOffset pagination driver
* [x] DataFusion execution
* [x] Postgres writer (merge by `id`)
* [x] Writer factory (no main-branching)
* [x] CLI with `--modules` / `--yaml-config`

**Pagination**

* [x] LimitOffset (`limit` + `offset`)
* [ ] PageNumber (`page` + `per_page`)
* [ ] PageOnly (`page`)
* [ ] Cursor (`cursor` tokens / next links)

**Next**

* [ ] ClickHouse writer
* [ ] BigQuery writer
* [ ] State for incremental loads
* [ ] Auth, retries/backoff
* [ ] Schema inference / evolution
* [ ] Logging/metrics + perf tuning
* [ ] Tests and CI

---

## 📚 Learning Notes

* Rust async with Tokio
* Traits + trait objects (`Arc<dyn DataWriter>`)
* DataFusion logical & physical plans
* Backpressure and pagination
* Clear module boundaries (`config/`, `pipeline/`, `cmd/`)

---

## 🤝 Contributing

New to Rust/data? Perfect—this is a learning repo.
PRs, ideas, docs, and questions are very welcome.

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

---

## 📄 License

MIT — see [LICENSE](LICENSE).