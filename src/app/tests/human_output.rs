use super::*;
use crate::model::{Identity, Post};

fn assert_terminal_safe(rendered: &str) {
    assert!(
        rendered
            .chars()
            .all(|character| { !character.is_control() || matches!(character, '\n' | '\t') })
    );
}

#[test]
fn human_post_escapes_terminal_controls_without_changing_json_content() {
    let text = "Unicode café\n\ttext\u{1b}]52;c;synthetic\u{7}\r\u{9b}2J\0";
    let mut output = Output::new("fxtwitter", None);
    output.posts.push(Post {
        id: "20".into(),
        author: Identity {
            id: "123".into(),
            handle: "fixture".into(),
        },
        text: text.into(),
        created_at: None,
        parent_id: None,
        parent_known: true,
        url: "https://x.com/fixture/status/20".into(),
    });
    let value = serde_json::to_value(output).unwrap();
    let rendered = super::human(&value);
    assert_terminal_safe(&rendered);
    assert!(rendered.contains("Unicode café\n\ttext"));
    assert!(rendered.contains("\\u{1b}") && rendered.contains("\\r"));
    assert_eq!(value["posts"][0]["text"], text);
}

#[test]
fn human_list_and_admin_output_do_not_emit_control_sequences() {
    let mut output = Output::new("graphql", Some("123".into()));
    let mut list = super::list_metadata::fixture();
    list.name = "Synthetic\u{1b}[2J".into();
    list.description = Some("Description\u{9b}2J\rhidden".into());
    output.lists = Some(vec![list]);
    assert_terminal_safe(&super::human(&serde_json::to_value(output).unwrap()));
    assert_terminal_safe(&super::human(&json!({"synthetic":"\u{1b}[2J"})));
}
