# 🚰 Apitap

**Stream JSON from REST APIs, transform with SQL, load into your warehouse**  
*Tiny HTTP-to-warehouse ETL engine powered by Apache DataFusion & Rust*

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)
[![DataFusion](https://img.shields.io/badge/powered%20by-DataFusion-blue)](https://datafusion.apache.org/)

**Quick links:**  
[What is Apitap?](#-what-is-apitap) • [Features](#-features) • [Installation](#-installation) • [Quick Start](#-quick-start) • [Architecture](#-architecture) • [Roadmap](#%EF%B8%8F-roadmap)

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

- Currently **tested with PostgreSQL 17+** only  
- Target compatibility is **PostgreSQL 14+**  
  - Plan: fall back to `ON CONFLICT` for PG \< 15 instead of `MERGE`  
- Expect breaking changes, rough edges, and sharp corners. Feedback and PRs are very welcome.

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
  - 📝 Other modes (`PageNumber`, `PageOnly`, `Cursor`) **planned**
- 🧠 **DataFusion-backed SQL execution**
- 🐘 **PostgreSQL writer**
  - Auto-create tables
  - Merge/upsert by primary key  
  - Uses Postgres 17+ today; compatibility work for 14–16 is planned
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
- 🔄 Better Postgres compatibility (14+ support)

Legend: ✅ Working • 🔄 In Progress • 📝 Planned

---

## 📦 Installation

Right now you install Apitap from source. The plan is to also ship **prebuilt binaries** so you can just download `apitap-run` and use it directly.

### Option 1 — Build from source (today)

Requirements:

- Rust toolchain (1.70+ recommended)
- PostgreSQL 17+ for best compatibility right now

Clone and build:

```bash
git clone https://github.com/yourusername/apitap.git
cd apitap

# Build a release binary
cargo build --release
````

This will produce `target/release/apitap-run`.

You can either run it in place:

```bash
./target/release/apitap-run --help
```

or put it somewhere on your `PATH`:

```bash
cp target/release/apitap-run /usr/local/bin/apitap-run
```

Then you can use:

```bash
apitap-run --modules ./pipelines --yaml-config ./pipelines.yaml
```

### Option 2 — Download binary (planned)

Planned workflow:

* Download a platform-specific binary from GitHub Releases:

  * `apitap-run-x86_64-unknown-linux-gnu`
  * `apitap-run-x86_64-pc-windows-msvc`
  * `apitap-run-aarch64-apple-darwin`
* Make it executable and put it on your `PATH`:

```bash
chmod +x apitap-run
mv apitap-run /usr/local/bin/apitap-run
apitap-run --help
```

This isn’t published yet, but the README is written so that when you start cutting releases, you just need to add the actual download links.

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
```

### 2) Prepare modules & config

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

### 3) Run through the binary

Once you’ve built or downloaded the binary:

```bash
apitap-run \
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

---

## 🛣️ Roadmap

**Core**

* [x] Minijinja modules + capture
* [x] LimitOffset pagination driver
* [x] DataFusion execution
* [x] Postgres writer (MERGE/upsert, tested on 17+)
* [x] Writer factory (no main-branching)
* [x] CLI with `--modules` / `--yaml-config`

**Postgres compatibility**

* [x] Tested on PostgreSQL 17+
* [ ] Verified on PostgreSQL 16
* [ ] Verified on PostgreSQL 15
* [ ] Compatibility layer for PostgreSQL 14+

  * Fall back to `ON CONFLICT` when `MERGE` isn’t available

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
* [ ] Release prebuilt binaries

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
apitap-run --modules ./pipelines --yaml-config ./pipelines.yaml
```

---

## 📄 License

MIT — see [LICENSE](LICENSE).