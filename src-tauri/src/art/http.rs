//! Minimal blocking HTTP for background jobs. Behind a trait so tests can
//! serve canned responses.

use std::time::Duration;

use crate::error::{Error, Result};

pub trait Fetch: Sync {
    /// GET `url`, returning the body. A 404 is `Ok(None)`.
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>>;
}

/// A response whose status the caller handles itself (API clients retry on
/// 401/429).
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait Post: Sync {
    /// POST `body` to `url`. Errors only for transport failures; any HTTP
    /// status comes back as a `Response`. `url` may carry credentials, so it
    /// never appears in errors or logs.
    fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response>;
}

pub struct Http {
    agent: ureq::Agent,
}

/// Larger responses are refused (cover PNGs are a few MB at most; the biggest
/// libretro index pages are a few MB of HTML).
const MAX_BODY: u64 = 32 * 1024 * 1024;

impl Http {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(60)))
            .user_agent(concat!("EmuFrontend/", env!("CARGO_PKG_VERSION"), " (personal game library)"))
            .build()
            .into();
        Self { agent }
    }
}

impl Fetch for Http {
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>> {
        match self.agent.get(url).call() {
            Ok(mut resp) => {
                let body = resp
                    .body_mut()
                    .with_config()
                    .limit(MAX_BODY)
                    .read_to_vec()
                    .map_err(|e| Error::msg(format!("reading {url}: {e}")))?;
                Ok(Some(body))
            }
            Err(ureq::Error::StatusCode(404)) => Ok(None),
            Err(e) => Err(Error::msg(format!("{url}: {e}"))),
        }
    }
}

impl Post for Http {
    fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response> {
        let host = url.split('/').nth(2).unwrap_or("server").to_string();
        let mut req = self.agent.post(url).config().http_status_as_error(false).build();
        for (k, v) in headers {
            req = req.header(*k, *v);
        }
        let mut resp = req.send(body).map_err(|e| Error::msg(format!("{host}: {}", scrub(&e.to_string()))))?;
        let status = resp.status().as_u16();
        let body = resp
            .body_mut()
            .with_config()
            .limit(MAX_BODY)
            .read_to_vec()
            .map_err(|e| Error::msg(format!("reading from {host}: {}", scrub(&e.to_string()))))?;
        Ok(Response { status, body })
    }
}

/// Drops anything that looks like a URL query (which may hold a client
/// secret) from transport error text.
fn scrub(s: &str) -> String {
    s.split_whitespace()
        .map(|w| match w.find('?') {
            Some(i) if w.contains("://") => &w[..i],
            _ => w,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
pub mod fake {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// Serves fixed bodies by URL; unknown URLs are 404s. Records requests.
    #[derive(Default)]
    pub struct FakeHttp {
        pub pages: HashMap<String, Vec<u8>>,
        pub requests: Mutex<Vec<String>>,
        pub offline: bool,
    }

    impl Fetch for FakeHttp {
        fn get(&self, url: &str) -> Result<Option<Vec<u8>>> {
            self.requests.lock().unwrap().push(url.to_string());
            if self.offline {
                return Err(Error::msg("offline"));
            }
            Ok(self.pages.get(url).cloned())
        }
    }

    /// A scripted POST server: `handler(url, headers, body)` answers each request.
    pub struct FakePost<F> {
        pub handler: F,
        pub requests: Mutex<Vec<(String, Vec<(String, String)>, String)>>,
    }

    impl<F: Fn(&str, &[(&str, &str)], &str) -> Response + Sync> FakePost<F> {
        pub fn new(handler: F) -> Self {
            Self { handler, requests: Mutex::default() }
        }
    }

    impl<F: Fn(&str, &[(&str, &str)], &str) -> Response + Sync> Post for FakePost<F> {
        fn post(&self, url: &str, headers: &[(&str, &str)], body: &str) -> Result<Response> {
            self.requests.lock().unwrap().push((
                url.to_string(),
                headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
                body.to_string(),
            ));
            Ok((self.handler)(url, headers, body))
        }
    }

    pub fn json(status: u16, v: serde_json::Value) -> Response {
        Response { status, body: serde_json::to_vec(&v).unwrap() }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn transport_errors_never_carry_a_query_string() {
        assert_eq!(
            super::scrub("io error connecting to https://id.twitch.tv/oauth2/token?client_secret=abc: refused"),
            "io error connecting to https://id.twitch.tv/oauth2/token refused"
        );
    }
}
