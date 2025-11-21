pub mod fetcher;
use datafusion::common::HashMap;
use reqwest::Client;

#[derive(Clone)]
pub struct Http {
    url: String,
    params: Option<HashMap<String, String>>,
    headers: Option<HashMap<String, String>>,
    bearer_auth: Option<String>,
}

impl Http {
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            params: None,
            headers: None,
            bearer_auth: None,
        }
    }
    pub fn param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let map = self.params.get_or_insert_with(HashMap::new);
        map.insert(key.into(), value.into());
        self
    }
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let map = self.headers.get_or_insert_with(HashMap::new);
        map.insert(key.into(), value.into());
        self
    }
    pub fn bearer_auth(mut self, token: impl Into<String>) -> Self {
        self.bearer_auth = Some(token.into());
        self
    }
    pub fn build_client(&self) -> Client {
        let mut headers = reqwest::header::HeaderMap::new();

        if let Some(header_map) = &self.headers {
            for (key, value) in header_map {
                if let (Ok(name), Ok(val)) = (
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()),
                    reqwest::header::HeaderValue::from_str(value),
                ) {
                    headers.insert(name, val);
                }
            }
        }
        if let Some(token) = &self.bearer_auth {
            match reqwest::header::HeaderValue::from_str(&format!("Bearer {}", token)) {
                Ok(header_value) => {
                    headers.insert(reqwest::header::AUTHORIZATION, header_value);
                }
                Err(_) => {
                    // Token contains invalid header characters, skip adding the header
                    // This prevents panic while still allowing the client to be built
                    eprintln!("Warning: Invalid characters in bearer token, skipping authorization header");
                }
            }
        }

        Client::builder()
            .default_headers(headers)
            // ===== HTTP Connection Pooling & Keep-Alive Optimizations =====
            // Based on flamegraph analysis: reduce TLS handshake overhead (6.48% CPU time)
            // Enable HTTP connection reuse and configure pool settings
            .pool_max_idle_per_host(10) // Keep up to 10 idle connections per host
            .pool_idle_timeout(Some(std::time::Duration::from_secs(90))) // Keep connections alive for 90s
            .timeout(std::time::Duration::from_secs(30)) // Request timeout
            .connect_timeout(std::time::Duration::from_secs(10)) // Connection timeout
            .tcp_keepalive(Some(std::time::Duration::from_secs(60))) // TCP keepalive
            // TLS session resumption is enabled by default in reqwest
            .build()
            .unwrap_or_else(|_| Client::new())
    }
    pub fn get_url(&self) -> String {
        if let Some(params) = &self.params {
            // keep any base params (we'll override limit/offset at call time)
            let query: Vec<String> = params
                .iter()
                .filter(|(k, _)| k.as_str() != "page") // ignore any page param
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();

            if query.is_empty() {
                self.url.clone()
            } else {
                format!("{}?{}", self.url, query.join("&"))
            }
        } else {
            self.url.clone()
        }
    }
}
