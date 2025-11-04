use http::Extensions;
use reqwest::{Client, Request, Response};
use reqwest_middleware::{
    ClientBuilder, ClientWithMiddleware, Middleware, Next, Result as MwResult,
};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use std::time::{Duration, Instant};
use tracing::{info, warn};

#[derive(Debug, Default, Clone)]
struct AttemptCount(pub u32);

struct AttemptLogger;

#[async_trait::async_trait]
impl Middleware for AttemptLogger {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> MwResult<Response> {
        let attempt = match extensions.get_mut::<AttemptCount>() {
            Some(c) => {
                c.0 += 1;
                c.0
            }
            None => {
                extensions.insert(AttemptCount(1));
                1
            }
        };

        let method = req.method().clone();
        let url = req.url().clone();
        let t0 = Instant::now();
        info!("→ attempt #{attempt} {method} {url}");

        let res = next.run(req, extensions).await;

        match &res {
            Ok(resp) => {
                let dt = t0.elapsed();
                info!(
                    "← attempt #{attempt} {} {} in {:?}",
                    resp.status(),
                    resp.url(),
                    dt
                );
            }
            Err(err) => {
                let dt = t0.elapsed();
                warn!("⇠ attempt #{attempt} error after {:?}: {err}", dt);
            }
        }
        res
    }
}

struct SummaryLogger;

#[async_trait::async_trait]
impl Middleware for SummaryLogger {
    async fn handle(
        &self,
        req: Request,
        extensions: &mut Extensions,
        next: Next<'_>,
    ) -> MwResult<Response> {
        let method = req.method().clone();
        let url = req.url().clone();
        let t0 = Instant::now();

        let res = next.run(req, extensions).await;

        let attempts = extensions.get::<AttemptCount>().map(|c| c.0).unwrap_or(1);
        match &res {
            Ok(resp) => info!(
                "✔ {method} {url} -> {} in {:?} (attempts: {attempts})",
                resp.status(),
                t0.elapsed()
            ),
            Err(err) => warn!(
                "✖ {method} {url} failed after {:?} (attempts: {attempts}): {err}",
                t0.elapsed()
            ),
        }
        res
    }
}

pub fn build_client_with_retry(reqwest_client: Client) -> ClientWithMiddleware {
    let policy = ExponentialBackoff::builder()
        .retry_bounds(Duration::from_millis(250), Duration::from_secs(8))
        .build_with_max_retries(6);

    let client = ClientBuilder::new(reqwest_client)
        .with(AttemptLogger)
        .with(RetryTransientMiddleware::new_with_policy(policy))
        .with(SummaryLogger)
        .build();

    client
}
