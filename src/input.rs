use crate::error::{Error, Kind, Result};
pub fn id(value: &str) -> Result<String> {
    if !value.is_empty()
        && value.len() <= 20
        && value.bytes().all(|b| b.is_ascii_digit())
        && value.parse::<u64>().is_ok_and(|v| v > 0)
    {
        return Ok(value.to_owned());
    }
    Err(Error::new(
        Kind::InvalidInput,
        "Expected a positive decimal post ID (at most 20 digits)",
    ))
}
pub fn handle(value: &str) -> Result<String> {
    let value = value.strip_prefix('@').unwrap_or(value);
    if !value.is_empty()
        && value.len() <= 15
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Ok(value.into());
    }
    Err(Error::new(Kind::InvalidInput, "Invalid X handle"))
}
pub fn post(value: &str) -> Result<String> {
    if value.split(['/', '?', '#']).any(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "." | ".." | "%2e" | "%2e%2e" | ".%2e" | "%2e."
        )
    }) {
        return Err(Error::new(
            Kind::InvalidInput,
            "Dot segments are not accepted in post URLs",
        ));
    }
    if value.bytes().all(|b| b.is_ascii_digit()) {
        return id(value);
    }
    let u = url::Url::parse(value)
        .map_err(|_| Error::new(Kind::InvalidInput, "Expected an X/Twitter post URL or ID"))?;
    if !matches!(u.scheme(), "https" | "http")
        || !matches!(
            u.host_str(),
            Some("x.com" | "www.x.com" | "twitter.com" | "www.twitter.com" | "mobile.twitter.com")
        )
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
    {
        return Err(Error::new(
            Kind::InvalidInput,
            "Unsupported post URL origin",
        ));
    }
    let p: Vec<_> = u.path().trim_end_matches('/').split('/').skip(1).collect();
    let raw = match p.as_slice() {
        [h, "status", i] => {
            handle(h)?;
            *i
        }
        ["i", "web", "status", i] => *i,
        [h, "status", i, "photo" | "video", n] if n.parse::<u8>().is_ok_and(|n| n > 0) => {
            handle(h)?;
            *i
        }
        _ => return Err(Error::new(Kind::InvalidInput, "Unsupported post URL path")),
    };
    id(raw)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls() {
        for v in [
            "20",
            "https://x.com/jack/status/20?s=1",
            "https://twitter.com/i/web/status/20",
            "https://x.com/jack/status/20/photo/1",
        ] {
            assert_eq!(post(v).unwrap(), "20");
        }
    }
    #[test]
    fn reject() {
        for v in [
            "",
            "0",
            "-1",
            "18446744073709551616",
            "https://x.com.evil/u/status/20",
            "https://x.com@evil/u/status/20",
            "https://x.com/u/status/20/extra",
            "https://x.com/u/status/%32%30",
            "https://x.com/u/status/20/../21",
        ] {
            assert!(post(v).is_err(), "{v}");
        }
    }
}
