//! Minimal blocking HTTP for background jobs. Behind a trait so tests can
//! serve canned responses.

use std::time::Duration;

use crate::error::{Error, Result};

pub trait Fetch: Sync {
    /// GET `url`, returning the body. A 404 is `Ok(None)`.
    fn get(&self, url: &str) -> Result<Option<Vec<u8>>>;
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
}
