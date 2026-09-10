use crate::{
    error::{Error, Kind, Result, protocol},
    model::{Identity, Output, Post},
    transport::{Request, Transport, check},
};
use serde_json::Value;

use crate::config::FXTWITTER_API_ORIGIN as API_ORIGIN;

pub fn read(t: &dyn Transport, id: &str) -> Result<Output> {
    crate::input::id(id)?;
    let response = t.get(Request::public(format!("{API_ORIGIN}/2/status/{id}")))?;
    check(&response, false)?;
    let v: Value = serde_json::from_slice(&response.body).map_err(|_| protocol())?;
    let code = v["code"].as_u64().ok_or_else(protocol)?;
    if code != 200 {
        check(
            &crate::transport::Response {
                status: u16::try_from(code).map_err(|_| protocol())?,
                body: vec![],
                retry_after: response.retry_after,
            },
            false,
        )?;
        return Err(protocol());
    }
    let p = parse(&v["status"])?;
    if p.id != id {
        return Err(protocol());
    }
    let mut out = Output::new("fxtwitter", None);
    out.posts.push(p);
    out.complete = true;
    out.stop_reason = "single_post".into();
    out.pages = 1;
    out.warnings
        .push("Third-party public source; source freshness and context are not guaranteed".into());
    Ok(out)
}
pub fn parse(v: &Value) -> Result<Post> {
    if v["type"] == "tombstone" {
        return Err(match v["reason"].as_str() {
            Some("private" | "blocked") => {
                Error::new(Kind::Permission, "Provider reports restricted post")
            }
            Some("deleted") => Error::new(Kind::Unavailable, "Provider reports deleted post"),
            _ => Error::new(
                Kind::Unavailable,
                "Provider reports unavailable post; reason uncertain",
            ),
        });
    }
    if v["type"] != "status" {
        return Err(protocol());
    }
    let string = |p: &str| {
        v.pointer(p)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(protocol)
    };
    let id = string("/id")?;
    crate::input::id(&id).map_err(|_| protocol())?;
    let author = Identity {
        id: string("/author/id")?,
        handle: string("/author/screen_name")?,
    };
    let parent_known = v.get("replying_to").is_some();
    let parent_id = match v.get("replying_to") {
        None | Some(Value::Null) => None,
        Some(r) => Some(
            crate::input::id(r["status"].as_str().ok_or_else(protocol)?).map_err(|_| protocol())?,
        ),
    };
    Ok(Post {
        url: format!("https://x.com/{}/status/{id}", author.handle),
        id,
        author,
        text: string("/text")?,
        created_at: v["created_at"].as_str().map(str::to_owned),
        parent_id,
        parent_known,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::Response;
    struct Mock;
    impl Transport for Mock {
        fn get(&self, r: Request) -> Result<Response> {
            assert!(r.headers.is_empty());
            assert!(r.url.ends_with("/2/status/20"));
            Ok(Response{status:200,body:br#"{"code":200,"status":{"type":"status","id":"20","text":"hello","author":{"id":"12","screen_name":"jack"},"replying_to":null}}"#.to_vec(),retry_after:None})
        }
    }
    #[test]
    fn vertical_slice() {
        let o = read(&Mock, "20").unwrap();
        assert_eq!(o.posts[0].id, "20");
        assert!(o.posts[0].parent_known);
        assert_eq!(serde_json::to_value(o).unwrap()["posts"][0]["id"], "20");
    }
    #[test]
    fn protocol_change() {
        assert!(parse(&serde_json::json!({"id":20})).is_err());
    }
    #[test]
    fn parent_unknown() {
        let p=parse(&serde_json::json!({"type":"status","id":"20","text":"","author":{"id":"12","screen_name":"jack"}})).unwrap();
        assert!(!p.parent_known);
    }
}
