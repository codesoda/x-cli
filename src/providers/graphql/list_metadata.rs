//! Single-list projection from the reviewed id_str/current-user normalizer.
use crate::{
    error::{Result, protocol},
    model::{ListInfo, ListVisibility},
};
use serde_json::Value;

pub(super) fn parse(value: &Value) -> Result<ListInfo> {
    // The list normalizer uses id_str, not a User/Tweet-style rest_id or typename.
    let id = value["id_str"].as_str().ok_or_else(protocol)?;
    crate::input::id(id).map_err(|_| protocol())?;
    let name = value["name"].as_str().ok_or_else(protocol)?;
    let description = optional_string(value, "description")?.map(str::to_owned);
    let visibility = match optional_string(value, "mode")? {
        None => None,
        Some(mode) if mode.eq_ignore_ascii_case("public") => Some(ListVisibility::Public),
        Some(mode) if mode.eq_ignore_ascii_case("private") => Some(ListVisibility::Private),
        Some(_) => return Err(protocol()),
    };
    let owner = match value.pointer("/user_results/result") {
        Some(user) if user["__typename"] == "User" => {
            // Require the current core branch before calling the shared validator;
            // never guess an owner using its historical legacy fallback.
            user.pointer("/core/screen_name")
                .and_then(Value::as_str)
                .ok_or_else(protocol)?;
            Some(super::parse_identity(user)?)
        }
        _ => None,
    };
    Ok(ListInfo {
        id: id.into(),
        name: name.into(),
        description,
        visibility,
        owner,
        url: format!("https://x.com/i/lists/{id}"),
    })
}

fn optional_string<'a>(value: &'a Value, field: &str) -> Result<Option<&'a str>> {
    match value.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        _ => Err(protocol()),
    }
}
