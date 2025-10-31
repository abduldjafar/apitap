use async_trait::async_trait;
use serde::{Deserialize, Deserializer, Serialize, de};
use sqlx::PgPool;
use std::collections::HashMap;

use crate::errors::Result as CustomResult;
use crate::http::fetcher::Pagination;

// ================== Public types ==================

#[derive(Debug, Clone, Serialize)]
pub struct Config {
    pub sources: Vec<Source>,
    pub targets: Vec<Target>,

    // name -> index (built on deserialize)
    #[serde(skip)]
    source_ix: HashMap<String, usize>,
    #[serde(skip)]
    target_ix: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub table_destination_name: Option<String>,
    #[serde(default)]
    pub pagination: Option<Pagination>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Target {
    Postgres(PostgresSink),
    // If/when you add BigQuery, add a variant here and extend `create_conn`.
}

pub enum TargetConn {
    Postgres { pool: PgPool, database: String },
}

#[async_trait]
pub trait SinkConn {
    async fn create_conn(&self) -> CustomResult<TargetConn>;
}

#[async_trait]
impl SinkConn for Target {
    async fn create_conn(&self) -> CustomResult<TargetConn> {
        match self {
            Target::Postgres(pg) => {
                let url = format!(
                    "postgres://{user}:{pass}@{host}:{port}/{db}",
                    user = pg.auth.username,
                    pass = pg.auth.password,
                    host = pg.host,
                    port = pg.port,
                    db = pg.database
                );
                let pool = PgPool::connect(&url).await?;
                Ok(TargetConn::Postgres {
                    pool,
                    database: pg.database.clone(),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresSink {
    pub name: String,
    pub host: String,
    #[serde(default = "default_pg_port")]
    pub port: u16,
    pub database: String,
    pub auth: PostgresAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgresAuth {
    pub username: String,
    pub password: String,
}

// (These are kept if you plan to add BigQuery later; otherwise you can remove.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BigQuerySink {
    pub name: String,
    pub dataset: String,
    pub auth: BigQueryAuth,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BigQueryAuth {
    pub service_account_path: String,
}

fn default_pg_port() -> u16 {
    5432
}

// ================== Deserialize with indexes ==================

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConfigWire {
    sources: Vec<Source>,
    targets: Vec<Target>,
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = ConfigWire::deserialize(deserializer)?;
        let mut cfg = Config {
            sources: wire.sources,
            targets: wire.targets,
            source_ix: HashMap::new(),
            target_ix: HashMap::new(),
        };
        cfg.build_indexes().map_err(de::Error::custom)?;
        Ok(cfg)
    }
}

// ================== Indexing & getters ==================

impl Config {
    fn build_indexes(&mut self) -> Result<(), String> {
        self.source_ix.clear();
        for (i, s) in self.sources.iter().enumerate() {
            if self.source_ix.insert(s.name.clone(), i).is_some() {
                return Err(format!("Duplicate source name: {}", s.name));
            }
        }
        self.target_ix.clear();
        for (i, t) in self.targets.iter().enumerate() {
            let key = t.name().to_string();
            if self.target_ix.insert(key.clone(), i).is_some() {
                return Err(format!("Duplicate target name: {key}"));
            }
        }
        Ok(())
    }

    /// Call this after any mutation that changes names or order.
    pub fn reindex(&mut self) -> Result<(), String> {
        self.build_indexes()
    }

    pub fn source(&self, name: &str) -> Option<&Source> {
        self.source_ix.get(name).and_then(|&i| self.sources.get(i))
    }
    pub fn source_mut(&mut self, name: &str) -> Option<&mut Source> {
        let i = *self.source_ix.get(name)?;
        self.sources.get_mut(i)
    }

    pub fn target(&self, name: &str) -> Option<&Target> {
        self.target_ix.get(name).and_then(|&i| self.targets.get(i))
    }
    pub fn target_mut(&mut self, name: &str) -> Option<&mut Target> {
        let i = *self.target_ix.get(name)?;
        self.targets.get_mut(i)
    }

    /// One-call helper: connect to a target by its unique name.
    pub async fn connect_sink(&self, name: &str) -> CustomResult<TargetConn> {
        let tgt = self
            .target(name)
            .ok_or_else(|| crate::errors::Error::Sqlx(format!("unknown target: {name}")))?;
        tgt.create_conn().await
    }
}

// Small helper so we can get a target’s name regardless of variant.
trait Named {
    fn name(&self) -> &str;
}
impl Named for Target {
    fn name(&self) -> &str {
        match self {
            Target::Postgres(x) => &x.name,
        }
    }
}

// ================== (Optional) MiniJinja helpers ==================
// Enable your templates to call `{{ source("json_place_holder") }}`
// and `{{ sink("postgres_sink") }}` to choose a YAML target by name.

pub mod run;
pub mod sink;
