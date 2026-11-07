use devdoctor_core::{diagnose, explain};
use serde_json::{Value, json};
use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};
fn main() {
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        let Ok(req): Result<Value, _> = serde_json::from_str(&line) else {
            continue;
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let result = match method {
            "initialize" => {
                json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"devdoctor","version":env!("CARGO_PKG_VERSION")}})
            }
            "notifications/initialized" => continue,
            "tools/list" => json!({"tools":tools()}),
            "tools/call" => call(req.get("params").unwrap_or(&Value::Null)),
            "ping" => json!({}),
            _ => json!({"error":{"code":-32601,"message":"method not found"}}),
        };
        let out = json!({"jsonrpc":"2.0","id":id,"result":result});
        println!("{}", out);
        io::stdout().flush().ok();
    }
}
fn tools() -> Value {
    json!([{"name":"diagnose_project","description":"Read-only project diagnostics","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}}},{"name":"check_toolchain","description":"Check local Rust and Node toolchains","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}}},{"name":"check_environment","description":"Check declared environment configuration","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}}},{"name":"check_ports","description":"Check project ports","inputSchema":{"type":"object","properties":{"path":{"type":"string"}}}},{"name":"explain_finding","description":"Explain a stable finding code","inputSchema":{"type":"object","properties":{"code":{"type":"string"}},"required":["code"]}}])
}
fn call(params: &Value) -> Value {
    let name = params.get("name").and_then(Value::as_str).unwrap_or("");
    let a = params.get("arguments").unwrap_or(&Value::Null);
    if name == "explain_finding" {
        let c = a.get("code").and_then(Value::as_str).unwrap_or("");
        return match explain(c) {
            Some((t, e)) => json!({"content":[{"type":"text","text":format!("{c}: {t}\n{e}")}]}),
            None => {
                json!({"isError":true,"content":[{"type":"text","text":"Unknown finding code"}]})
            }
        };
    }
    let p = PathBuf::from(a.get("path").and_then(Value::as_str).unwrap_or("."));
    if !p.is_dir() {
        return json!({"isError":true,"content":[{"type":"text","text":"path must be an existing directory"}]});
    }
    let r = diagnose(&p);
    json!({"content":[{"type":"text","text":serde_json::to_string(&r).unwrap()}]})
}
