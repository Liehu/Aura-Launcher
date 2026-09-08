fn main() {
    launcher_plugin_api::serve(|q| {
        serde_json::json!([
            { "title": format!("normal: {q}"), "subtitle": "ok", "actions": ["noop"] }
        ])
    })
    .unwrap();
}
