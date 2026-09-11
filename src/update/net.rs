//! Bounded HTTPS transport for public GitHub release metadata and assets.
//! Separate from the X transport by design: it never carries session
//! material, and only ever issues anonymous GET requests.
use super::{Releases, release};
use crate::error::{Error, Kind, Result};
use std::io::Read;
use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const RESOLVE_TIMEOUT: Duration = Duration::from_secs(30);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_REDIRECTS: usize = 5;

pub struct GithubReleases {
    resolve: reqwest::blocking::Client,
    download: reqwest::blocking::Client,
}

impl GithubReleases {
    pub fn new() -> Result<Self> {
        let resolve = builder(RESOLVE_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(client_error)?;
        // Asset downloads redirect to GitHub's CDN; follow a bounded number
        // of redirects and refuse any hop that leaves HTTPS.
        let download = builder(DOWNLOAD_TIMEOUT)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.url().scheme() != "https" {
                    attempt.error("redirect left HTTPS")
                } else if attempt.previous().len() > MAX_REDIRECTS {
                    attempt.error("too many redirects")
                } else {
                    attempt.follow()
                }
            }))
            .build()
            .map_err(client_error)?;
        Ok(Self { resolve, download })
    }
}

fn builder(timeout: Duration) -> reqwest::blocking::ClientBuilder {
    reqwest::blocking::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
        .user_agent(concat!(
            "xcli/",
            env!("CARGO_PKG_VERSION"),
            " (update; ",
            env!("CARGO_PKG_REPOSITORY"),
            ")"
        ))
}

fn client_error(_: reqwest::Error) -> Error {
    Error::new(Kind::Network, "Cannot initialize the update HTTPS client")
}

impl Releases for GithubReleases {
    fn latest_location(&self) -> Result<String> {
        let response = self
            .resolve
            .get(release::latest_url())
            .send()
            .map_err(|_| {
                Error::new(
                    Kind::Network,
                    "Cannot reach GitHub to check the latest release",
                )
            })?;
        if !response.status().is_redirection() {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "GitHub latest-release lookup did not return the expected redirect",
            ));
        }
        response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| {
                Error::new(
                    Kind::ProtocolChanged,
                    "GitHub latest-release redirect is missing a usable location",
                )
            })
    }

    fn download(&self, url: &str, max_bytes: u64) -> Result<Vec<u8>> {
        let parsed = url::Url::parse(url)
            .ok()
            .filter(|u| u.scheme() == "https")
            .ok_or_else(|| Error::new(Kind::Unsupported, "Update download origin rejected"))?;
        let response = self.download.get(parsed).send().map_err(|_| {
            Error::new(
                Kind::Network,
                "Cannot download the release asset from GitHub",
            )
        })?;
        if !response.status().is_success() {
            return Err(Error::new(
                Kind::Unavailable,
                "Release asset is not available from GitHub",
            ));
        }
        let mut body = Vec::new();
        response
            .take(max_bytes + 1)
            .read_to_end(&mut body)
            .map_err(|_| Error::new(Kind::Network, "Cannot read the release asset download"))?;
        if body.len() as u64 > max_bytes {
            return Err(Error::new(
                Kind::ProtocolChanged,
                "Release asset exceeds the supported download size",
            ));
        }
        Ok(body)
    }
}
