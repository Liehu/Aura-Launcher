use launcher_mcp::transport::McpTransport;
fn main() {
    let exe = std::env::args().nth(1).expect("usage: debug_stdio <exe>");
    let mut t = launcher_mcp::transport::stdio::StdioTransport::spawn(
        &exe, &[], std::time::Duration::from_secs(3),
    ).expect("spawn");
    let init = t.initialize().expect("initialize");
    println!("init ok: {:?}", init.server_info);
    let tools = t.list_tools().expect("list_tools");
    println!("tools: {:?}", tools.tools.iter().map(|t| &t.name).collect::<Vec<_>>());
    let call = t.call_tool("evaluate", serde_json::json!({"expression": "6 * 7"}), "e-1").expect("call");
    println!("call: {:?}", call.content);
    t.shutdown();
}
