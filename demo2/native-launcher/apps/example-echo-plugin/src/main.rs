//! Example external plugin: echoes the query back as results using the
//! Native UI Schema (list items with title/subtitle/actions).

fn main() {
    launcher_plugin_api::serve(|query| {
        serde_json::json!([
            {
                "title": format!("Echo: {query}"),
                "subtitle": "from example-echo-plugin",
                "actions": ["echo"]
            },
            {
                "title": "Echo uppercase",
                "subtitle": query.to_uppercase(),
                "actions": ["echo"]
            }
        ])
    })
    .expect("plugin io");
}
