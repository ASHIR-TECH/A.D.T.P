use reqwest::Client as HttpClient;
use std::time::Duration;

use crate::config::CliConfig;
use crate::errors::CliError;

pub struct ApiClient {
    client: HttpClient,
    base_url: String,
    token: String,
}

impl ApiClient {
    pub fn new(cfg: &CliConfig) -> Result<Self, CliError> {
        let connect_timeout = Duration::from_millis(cfg.connect_timeout_ms);
        let request_timeout = Duration::from_millis(cfg.request_timeout_ms);

        let client = HttpClient::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .build()
            .map_err(|e| CliError::Http { message: e.to_string() })?;

        Ok(Self {
            client,
            base_url: cfg.api_url.trim_end_matches('/').to_string(),
            token: cfg.api_token.clone(),
        })
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        h.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        h
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, CliError> {
        let resp = self
            .client
            .get(&self.url(path))
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| map_http_error(e, &self.base_url))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_api_error(status, &body));
        }

        resp.json::<T>().await.map_err(|e| CliError::Http {
            message: format!("Failed to parse response: {}", e),
        })
    }

    pub async fn get_with_query<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, CliError> {
        let resp = self
            .client
            .get(&self.url(path))
            .headers(self.headers())
            .query(params)
            .send()
            .await
            .map_err(|e| map_http_error(e, &self.base_url))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_api_error(status, &body));
        }

        resp.json::<T>().await.map_err(|e| CliError::Http {
            message: format!("Failed to parse response: {}", e),
        })
    }

    pub async fn get_raw(&self, path: &str) -> Result<String, CliError> {
        let resp = self
            .client
            .get(&self.url(path))
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| map_http_error(e, &self.base_url))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_api_error(status, &body));
        }

        resp.text().await.map_err(|e| CliError::Http {
            message: e.to_string(),
        })
    }

    pub async fn get_raw_with_query(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<String, CliError> {
        let resp = self
            .client
            .get(&self.url(path))
            .headers(self.headers())
            .query(params)
            .send()
            .await
            .map_err(|e| map_http_error(e, &self.base_url))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_api_error(status, &body));
        }

        resp.text().await.map_err(|e| CliError::Http {
            message: e.to_string(),
        })
    }

    pub async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<T, CliError> {
        let resp = self
            .client
            .post(&self.url(path))
            .headers(self.headers())
            .json(body)
            .send()
            .await
            .map_err(|e| map_http_error(e, &self.base_url))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_api_error(status, &body));
        }

        resp.json::<T>().await.map_err(|e| CliError::Http {
            message: format!("Failed to parse response: {}", e),
        })
    }

    pub async fn delete(&self, path: &str) -> Result<(), CliError> {
        let resp = self
            .client
            .delete(&self.url(path))
            .headers(self.headers())
            .send()
            .await
            .map_err(|e| map_http_error(e, &self.base_url))?;

        let status = resp.status().as_u16();
        if status >= 400 {
            let body = resp.text().await.unwrap_or_default();
            return Err(map_api_error(status, &body));
        }

        Ok(())
    }
}

fn map_http_error(e: reqwest::Error, base_url: &str) -> CliError {
    if e.is_connect() || e.is_timeout() {
        CliError::Http {
            message: format!(
                "Cannot connect to ADTP API at {base_url}. Is adtp-api running?"
            ),
        }
    } else {
        CliError::Http {
            message: e.to_string(),
        }
    }
}

fn map_api_error(status: u16, body: &str) -> CliError {
    let detail = extract_detail(body);
    match status {
        401 => CliError::Api {
            status,
            message: "Invalid API token. Check ADTP_API_TOKEN.".into(),
        },
        404 => CliError::Api { status, message: detail },
        413 => CliError::Api {
            status,
            message: format!("File exceeds size limit. {}", detail),
        },
        429 => CliError::Api {
            status,
            message: format!("Too many requests. Retry later. {}", detail),
        },
        503 => CliError::Api {
            status,
            message: format!("Agent daemon unreachable. Is adtp-client running? {}", detail),
        },
        _ => CliError::Api { status, message: detail },
    }
}

fn extract_detail(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        v.get("detail")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| body.to_string())
    } else {
        body.to_string()
    }
}
