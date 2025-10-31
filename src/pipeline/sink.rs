use std::pin::Pin;
// src/pipeline/sink.rs (new file or tuck into templating.rs’ neighbor)
use std::sync::Arc;

use crate::pipeline::TargetConn;
use crate::writer::postgres::PostgresWriter;
use crate::writer::{DataWriter, WriteMode};
use crate::errors::{Result,Error};

pub type HookFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;
pub type Hook       = Box<dyn FnOnce() -> HookFuture + Send>;

#[derive(Debug, Clone)]
pub struct WriterOpts<'a> {
    pub dest_table: &'a str,
    pub primary_key: &'a str,
    pub batch_size: usize,
    pub sample_size: usize,
    pub auto_create: bool,
    pub auto_truncate: bool,
    pub truncate_first: bool,
    pub write_mode: WriteMode,
}

pub trait MakeWriter {
    fn make_writer(&self, opts: &WriterOpts<'_>) -> Result<(Arc<dyn DataWriter>, Option<Hook>)>;
}

impl MakeWriter for TargetConn {
    fn make_writer(&self, opts: &WriterOpts<'_>) -> Result<(Arc<dyn DataWriter>, Option<Hook>)> {
        match self {
            TargetConn::Postgres { pool, .. } => {
                // 1) Build concrete writer
                let pg = Arc::new(
                    PostgresWriter::new(pool.clone(), opts.dest_table)
                        .with_primary_key_single(opts.primary_key)
                        .with_batch_size(opts.batch_size)
                        .with_sample_size(opts.sample_size)
                        .auto_create(opts.auto_create)
                        .auto_truncate(opts.auto_truncate),
                );

                // 2) Optional truncate hook that captures the *concrete* writer
                let hook: Option<Hook> = if opts.truncate_first {
                    let pg_for_hook = Arc::clone(&pg);
                    Some(Box::new(move || {
                        Box::pin(async move {
                            pg_for_hook.truncate().await?;
                            Ok(())
                        }) as HookFuture
                    }))
                } else {
                    None
                };

                // 3) Upcast to trait object
                let writer: Arc<dyn DataWriter> = pg;

                Ok((writer, hook))
            }

            _ => Err(Error::Reqwest("unsupported sink".into())),
        }
    }
}
