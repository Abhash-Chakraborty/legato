//! The Model Context Protocol over stdio, as `legato --mcp` speaks it: one
//! JSON-RPC message per line in, one per line out. This part answers
//! messages; what the tools do is behind [`Tools`], so it's tested without
//! the app or its database.

use serde_json::{json, Value};

/// Protocol versions this server speaks, newest first.
pub const VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// A tool as `tools/list` describes it.
pub struct ToolSpec {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema for the arguments.
    pub input_schema: Value,
}

/// What the tools do. `call` returns the text the AI app reads, or an
/// error it reads instead.
pub trait Tools {
    fn specs(&self) -> Vec<ToolSpec>;
    fn call(&self, name: &str, args: &Value) -> Result<String, String>;
}

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

fn reply(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error(id: &Value, code: i64, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
    .to_string()
}

/// Answers one line. Notifications (no id) get no answer, so this returns
/// None for them and for blank lines.
pub fn handle(line: &str, tools: &dyn Tools, server_version: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let msg: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Some(error(&Value::Null, PARSE_ERROR, "Not JSON")),
    };
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        // A response to something we never asked, or garbage: say so only
        // when it carries an id to answer.
        return msg
            .get("id")
            .map(|id| error(id, INVALID_REQUEST, "No method"));
    };
    let id = msg.get("id")?;
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    Some(match method {
        "initialize" => {
            let asked = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or(VERSIONS[0]);
            let version = VERSIONS
                .iter()
                .find(|v| **v == asked)
                .copied()
                .unwrap_or(VERSIONS[0]);
            reply(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": { "name": "legato", "title": "Legato", "version": server_version },
                    "instructions": "Legato is a speech-to-text app on this PC. These tools read the person's dictations, meeting and voice notes, Dictionary, Scratchpad and calendar meetings, and can add a Scratchpad note.",
                }),
            )
        }
        "ping" => reply(id, json!({})),
        "tools/list" => {
            let tools: Vec<Value> = tools
                .specs()
                .into_iter()
                .map(|t| {
                    json!({
                        "name": t.name,
                        "description": t.description,
                        "inputSchema": t.input_schema,
                    })
                })
                .collect();
            reply(id, json!({ "tools": tools }))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return Some(error(id, INVALID_PARAMS, "Missing tool name"));
            };
            if !tools.specs().iter().any(|t| t.name == name) {
                return Some(error(id, INVALID_PARAMS, &format!("Unknown tool: {name}")));
            }
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            match tools.call(name, &args) {
                Ok(text) => reply(
                    id,
                    json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
                ),
                Err(text) => reply(
                    id,
                    json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
                ),
            }
        }
        // Listed so clients that ask find nothing rather than an error.
        "resources/list" => reply(id, json!({ "resources": [] })),
        "prompts/list" => reply(id, json!({ "prompts": [] })),
        _ => error(id, METHOD_NOT_FOUND, &format!("Unknown method: {method}")),
    })
}

/// A whole-number argument, clamped, or the default when it's missing.
pub fn arg_u32(args: &Value, key: &str, default: u32, max: u32) -> u32 {
    args.get(key)
        .and_then(Value::as_u64)
        .map(|n| (n as u32).clamp(1, max))
        .unwrap_or(default)
}

pub fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake;
    impl Tools for Fake {
        fn specs(&self) -> Vec<ToolSpec> {
            vec![ToolSpec {
                name: "echo",
                description: "Says it back",
                input_schema: json!({ "type": "object", "properties": { "text": { "type": "string" } } }),
            }]
        }
        fn call(&self, _name: &str, args: &Value) -> Result<String, String> {
            arg_str(args, "text")
                .map(str::to_string)
                .ok_or_else(|| "Nothing to say".to_string())
        }
    }

    fn ask(line: &str) -> Value {
        serde_json::from_str(&handle(line, &Fake, "1.0").expect("an answer")).unwrap()
    }

    #[test]
    fn initialize_agrees_on_a_version() {
        let r = ask(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#,
        );
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        assert_eq!(r["result"]["serverInfo"]["name"], "legato");
        let r = ask(
            r#"{"jsonrpc":"2.0","id":2,"method":"initialize","params":{"protocolVersion":"2099-01-01"}}"#,
        );
        assert_eq!(r["result"]["protocolVersion"], VERSIONS[0]);
    }

    #[test]
    fn notifications_get_no_answer() {
        assert!(handle(
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            &Fake,
            "1"
        )
        .is_none());
        assert!(handle("   ", &Fake, "1").is_none());
    }

    #[test]
    fn tools_are_listed_and_called() {
        let r = ask(r#"{"jsonrpc":"2.0","id":"a","method":"tools/list"}"#);
        assert_eq!(r["id"], "a");
        assert_eq!(r["result"]["tools"][0]["name"], "echo");
        assert!(r["result"]["tools"][0]["inputSchema"].is_object());

        let r = ask(
            r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"text":"Dokploy"}}}"#,
        );
        assert_eq!(r["result"]["content"][0]["text"], "Dokploy");
        assert_eq!(r["result"]["isError"], false);

        let r = ask(
            r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"echo","arguments":{}}}"#,
        );
        assert_eq!(r["result"]["isError"], true);

        let r = ask(r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"nope"}}"#);
        assert_eq!(r["error"]["code"], INVALID_PARAMS);
    }

    #[test]
    fn bad_input_gets_errors() {
        assert_eq!(ask("{not json")["error"]["code"], PARSE_ERROR);
        assert_eq!(
            ask(r#"{"jsonrpc":"2.0","id":6,"method":"sampling/x"}"#)["error"]["code"],
            METHOD_NOT_FOUND
        );
    }

    #[test]
    fn arguments_are_clamped() {
        let args = json!({ "limit": 5000, "query": "  " });
        assert_eq!(arg_u32(&args, "limit", 20, 100), 100);
        assert_eq!(arg_u32(&json!({}), "limit", 20, 100), 20);
        assert_eq!(arg_str(&args, "query"), None);
    }
}
