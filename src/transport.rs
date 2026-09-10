use crate::error::{Error, Kind, Result, protocol};
use std::io::Read;
use zeroize::Zeroizing;

/// Intentionally not Debug/Serialize. Sensitive headers are zeroed on drop.
pub struct Request {
    pub url: String,
    pub headers: Vec<(String, Zeroizing<String>)>,
}
impl Request {
    pub fn public(url: String) -> Self {
        Self {
            url,
            headers: vec![],
        }
    }
}
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub retry_after: Option<u64>,
}
pub trait Transport {
    fn get(&self, request: Request) -> Result<Response>;
}
pub struct Http {
    client: reqwest::blocking::Client,
}
impl Http {
    pub fn new() -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("xcli/0.1.0 (read-only; https://github.com/codesoda/x-cli)")
            .build()
            .map_err(|_| Error::new(Kind::Network, "Cannot initialize HTTPS transport"))?;
        Ok(Self { client })
    }
}
impl Transport for Http {
    fn get(&self, request: Request) -> Result<Response> {
        let u = url::Url::parse(&request.url).map_err(|_| protocol())?;
        if u.scheme() != "https"
            || !matches!(
                u.host_str(),
                Some("x.com" | "api.fxtwitter.com" | "abs.twimg.com")
            )
            || (!request.headers.is_empty() && u.host_str() != Some("x.com"))
        {
            return Err(Error::new(Kind::Unsupported, "Transport origin rejected"));
        }
        let mut builder = self.client.get(u);
        for (name, value) in &request.headers {
            let mut h = reqwest::header::HeaderValue::from_str(value)
                .map_err(|_| Error::new(Kind::Authentication, "Invalid session material"))?;
            h.set_sensitive(true);
            builder = builder.header(name, h);
        }
        let response = builder
            .send()
            .map_err(|_| Error::new(Kind::Network, "HTTPS request failed (details redacted)"))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| retry_delay(v, std::time::SystemTime::now()))
            .or_else(|| {
                response
                    .headers()
                    .get("x-rate-limit-reset")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .map(|v| v.saturating_sub(crate::model::now()))
            });
        let mut body = vec![];
        response
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut body)
            .map_err(|_| Error::new(Kind::Network, "Cannot read HTTPS response"))?;
        if body.len() > 8 * 1024 * 1024 {
            return Err(protocol());
        }
        Ok(Response {
            status,
            body,
            retry_after,
        })
    }
}
fn retry_delay(value: &str, now: std::time::SystemTime) -> Option<u64> {
    value.parse::<u64>().ok().or_else(|| {
        let until = httpdate::parse_http_date(value).ok()?;
        let delay = until.duration_since(now).unwrap_or_default();
        Some(
            delay
                .as_secs()
                .saturating_add(u64::from(delay.subsec_nanos() != 0)),
        )
    })
}
pub fn check(response: &Response, authenticated: bool) -> Result<()> {
    let e = match response.status {
        200..=299 => return Ok(()),
        401 if authenticated => Error::new(
            Kind::Authentication,
            "X session expired or rejected; reconnect explicitly",
        ),
        401 | 403 => Error::new(
            Kind::Permission,
            "Upstream denied access; exact cause may be unavailable",
        ),
        404 => Error::new(
            Kind::Unavailable,
            "Not available from this provider; deletion is not established",
        ),
        429 => {
            let mut e = Error::new(
                Kind::RateLimit,
                "Rate limited; wait before retrying; no automatic retry performed",
            );
            e.retry_after_seconds = Some(response.retry_after.unwrap_or(60).max(1));
            e
        }
        500..=599 => Error::new(Kind::Network, "Upstream service failure"),
        _ => protocol(),
    };
    Err(e)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retry_after_seconds_and_http_dates() {
        let now = std::time::UNIX_EPOCH + std::time::Duration::from_secs(100);
        assert_eq!(retry_delay("42", now), Some(42));
        let future = httpdate::fmt_http_date(now + std::time::Duration::from_secs(120));
        assert_eq!(retry_delay(&future, now), Some(120));
        assert_eq!(retry_delay("not-a-date", now), None);
    }
    #[test]
    fn safe_errors() {
        for (status, kind) in [
            (401, Kind::Authentication),
            (403, Kind::Permission),
            (429, Kind::RateLimit),
            (400, Kind::ProtocolChanged),
            (503, Kind::Network),
        ] {
            let e = check(
                &Response {
                    status,
                    body: b"auth_token=secret csrf=secret".to_vec(),
                    retry_after: Some(42),
                },
                true,
            )
            .unwrap_err();
            assert_eq!(e.kind, kind);
            assert!(!format!("{e:?}").contains("secret"));
            if status == 429 {
                assert_eq!(e.retry_after_seconds, Some(42));
            }
        }
    }
}
