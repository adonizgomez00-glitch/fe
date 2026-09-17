#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── JSON-RPC 2.0 Core Types ───────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        }
    }

    pub fn prompt(id: u64, params: PromptParams) -> Self {
        Self::new(id, "prompt", Some(serde_json::to_value(params).unwrap()))
    }

    pub fn session_list(id: u64) -> Self {
        Self::new(id, "session.list", None)
    }

    pub fn session_show(id: u64, session_id: &str) -> Self {
        let params = SessionShowParams { id: session_id.into() };
        Self::new(id, "session.show", Some(serde_json::to_value(params).unwrap()))
    }

    pub fn health(id: u64) -> Self {
        Self::new(id, "health", None)
    }

    pub fn config_get(id: u64) -> Self {
        Self::new(id, "config.get", None)
    }

    pub fn parse_method(&self) -> Result<DaemonMethod, String> {
        self.method.parse()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: u64, code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data,
            }),
        }
    }

    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// ─── Standard JSON-RPC Error Codes ─────────────────────────────

pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    pub const DAEMON_NOT_RUNNING: i32 = -32000;
    pub const SESSION_NOT_FOUND: i32 = -32001;
    pub const METHOD_NOT_IMPLEMENTED: i32 = -32002;
}

// ─── Method Enum ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DaemonMethod {
    Prompt,
    SessionList,
    SessionShow,
    ConfigGet,
    ConfigSet,
    McpDiscover,
    McpServers,
    PluginsList,
    PluginsReload,
    Health,
}

impl DaemonMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Prompt => "prompt",
            Self::SessionList => "session.list",
            Self::SessionShow => "session.show",
            Self::ConfigGet => "config.get",
            Self::ConfigSet => "config.set",
            Self::McpDiscover => "mcp.discover",
            Self::McpServers => "mcp.servers",
            Self::PluginsList => "plugins.list",
            Self::PluginsReload => "plugins.reload",
            Self::Health => "health",
        }
    }
}

impl std::str::FromStr for DaemonMethod {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "prompt" => Ok(Self::Prompt),
            "session.list" => Ok(Self::SessionList),
            "session.show" => Ok(Self::SessionShow),
            "config.get" => Ok(Self::ConfigGet),
            "config.set" => Ok(Self::ConfigSet),
            "mcp.discover" => Ok(Self::McpDiscover),
            "mcp.servers" => Ok(Self::McpServers),
            "plugins.list" => Ok(Self::PluginsList),
            "plugins.reload" => Ok(Self::PluginsReload),
            "health" => Ok(Self::Health),
            _ => Err(format!("Unknown method: {}", s)),
        }
    }
}

impl std::fmt::Display for DaemonMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ─── Method-specific Params ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptParams {
    pub text: String,
    #[serde(default)]
    pub flags: PromptFlags,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptFlags {
    #[serde(default)]
    pub batch: bool,
    #[serde(default)]
    pub reasoning: bool,
    #[serde(default)]
    pub fast: bool,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default)]
    pub destructive: bool,
    #[serde(default)]
    pub checkpoint: bool,
}

impl Default for PromptFlags {
    fn default() -> Self {
        Self {
            batch: false,
            reasoning: false,
            fast: false,
            dry_run: false,
            destructive: false,
            checkpoint: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionShowParams {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetParams {
    pub key: String,
    pub value: Value,
}

// ─── Streaming Events (daemon → client) ────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonEvent {
    #[serde(rename = "token")]
    Token {
        content: String,
    },
    #[serde(rename = "tool_call")]
    ToolCall {
        tool: String,
        #[serde(rename = "params")]
        tool_params: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool: String,
        output: String,
    },
    #[serde(rename = "done")]
    Done {
        session_id: String,
    },
    #[serde(rename = "error")]
    EventError {
        code: i32,
        message: String,
    },
}

impl DaemonEvent {
    pub fn to_response(&self, id: u64) -> JsonRpcResponse {
        JsonRpcResponse::success(id, serde_json::to_value(self).unwrap())
    }

    pub fn to_json_line(&self, id: u64) -> String {
        let resp = self.to_response(id);
        serde_json::to_string(&resp).unwrap()
    }
}

// ─── Result Types ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResult {
    pub status: String,
    pub version: String,
    pub ollama_connected: bool,
    pub sessions_count: Option<usize>,
    pub skills_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpDiscoverResult {
    pub servers: Vec<McpServerStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerStatus {
    pub name: String,
    pub enabled: bool,
    pub connected: bool,
    pub tools_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsListResult {
    pub plugins: Vec<PluginStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginStatus {
    pub name: String,
    pub version: String,
    pub enabled: bool,
    pub tools_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionExportParams {
    pub id: String,
    pub output: String,
}

// ─── Serialization Helpers ─────────────────────────────────────

pub fn parse_request(data: &str) -> Result<JsonRpcRequest, JsonRpcResponse> {
    serde_json::from_str::<JsonRpcRequest>(data).map_err(|e| {
        JsonRpcResponse::error(0, error_codes::PARSE_ERROR, &e.to_string(), None)
    })
}

pub fn parse_event(data: &str) -> Result<DaemonEvent, String> {
    serde_json::from_str::<DaemonEvent>(data).map_err(|e| e.to_string())
}

pub fn serialize_response(resp: &JsonRpcResponse) -> String {
    serde_json::to_string(resp).unwrap_or_else(|_| {
        r#"{"jsonrpc":"2.0","id":0,"error":{"code":-32603,"message":"Serialization error"}}"#.into()
    })
}

// ─── Line-delimited JSON helpers ───────────────────────────────

pub fn extract_line(buffer: &str) -> Option<(String, &str)> {
    if let Some(newline_pos) = buffer.find('\n') {
        let line = buffer[..newline_pos].trim().to_string();
        let rest = &buffer[newline_pos + 1..];
        Some((line, rest))
    } else {
        None
    }
}

pub fn extract_lines(buffer: &str) -> (Vec<String>, &str) {
    let mut lines = Vec::new();
    let mut remaining = buffer;
    while let Some((line, rest)) = extract_line(remaining) {
        if !line.is_empty() {
            lines.push(line);
        }
        remaining = rest;
    }
    (lines, remaining)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Request serialization/deserialization ─────────────────

    #[test]
    fn test_request_serialize() {
        let req = JsonRpcRequest::new(1, "prompt", None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"prompt\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_request_deserialize() {
        let json = r#"{"jsonrpc":"2.0","id":42,"method":"health"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.id, 42);
        assert_eq!(req.method, "health");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_request_with_params() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"prompt","params":{"text":"hola","flags":{"batch":true}}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "prompt");
        let params: PromptParams = serde_json::from_value(req.params.unwrap()).unwrap();
        assert_eq!(params.text, "hola");
        assert!(params.flags.batch);
        assert!(!params.flags.reasoning);
    }

    #[test]
    fn test_request_prompt_builder() {
        let params = PromptParams {
            text: "actualiza repositorios".into(),
            flags: PromptFlags { batch: true, ..Default::default() },
        };
        let req = JsonRpcRequest::prompt(5, params);
        assert_eq!(req.id, 5);
        assert_eq!(req.method, "prompt");
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("actualiza repositorios"));
        assert!(json.contains("\"batch\":true"));
    }

    #[test]
    fn test_request_session_list_builder() {
        let req = JsonRpcRequest::session_list(10);
        assert_eq!(req.method, "session.list");
        assert!(req.params.is_none());
    }

    #[test]
    fn test_request_session_show_builder() {
        let req = JsonRpcRequest::session_show(3, "abc-123");
        assert_eq!(req.method, "session.show");
        let params: SessionShowParams = serde_json::from_value(req.params.unwrap()).unwrap();
        assert_eq!(params.id, "abc-123");
    }

    #[test]
    fn test_request_health_builder() {
        let req = JsonRpcRequest::health(1);
        assert_eq!(req.method, "health");
    }

    // ─── Response serialization/deserialization ────────────────

    #[test]
    fn test_response_success() {
        let resp = JsonRpcResponse::success(1, serde_json::json!({"status":"ok"}));
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"result\":{\"status\":\"ok\"}"));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_response_error() {
        let resp = JsonRpcResponse::error(1, error_codes::METHOD_NOT_FOUND, "Method not found", None);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"error\""));
        assert!(json.contains("\"code\":-32601"));
    }

    #[test]
    fn test_response_deserialize_success() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"status":"ok"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.id, 1);
        assert!(resp.result.is_some());
        assert!(resp.error.is_none());
        assert!(!resp.is_error());
    }

    #[test]
    fn test_response_deserialize_error() {
        let json = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"Method not found"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(resp.result.is_none());
        assert!(resp.error.is_some());
        assert!(resp.is_error());
        let err = resp.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    // ─── Method enum ───────────────────────────────────────────

    #[test]
    fn test_method_from_str_valid() {
        assert_eq!("prompt".parse::<DaemonMethod>().unwrap(), DaemonMethod::Prompt);
        assert_eq!("session.list".parse::<DaemonMethod>().unwrap(), DaemonMethod::SessionList);
        assert_eq!("session.show".parse::<DaemonMethod>().unwrap(), DaemonMethod::SessionShow);
        assert_eq!("config.get".parse::<DaemonMethod>().unwrap(), DaemonMethod::ConfigGet);
        assert_eq!("config.set".parse::<DaemonMethod>().unwrap(), DaemonMethod::ConfigSet);
        assert_eq!("mcp.discover".parse::<DaemonMethod>().unwrap(), DaemonMethod::McpDiscover);
        assert_eq!("plugins.list".parse::<DaemonMethod>().unwrap(), DaemonMethod::PluginsList);
        assert_eq!("health".parse::<DaemonMethod>().unwrap(), DaemonMethod::Health);
    }

    #[test]
    fn test_method_from_str_invalid() {
        let result = "unknown".parse::<DaemonMethod>();
        assert!(result.is_err());
    }

    #[test]
    fn test_method_as_str() {
        assert_eq!(DaemonMethod::Prompt.as_str(), "prompt");
        assert_eq!(DaemonMethod::SessionList.as_str(), "session.list");
        assert_eq!(DaemonMethod::Health.as_str(), "health");
    }

    #[test]
    fn test_method_display() {
        assert_eq!(format!("{}", DaemonMethod::Prompt), "prompt");
        assert_eq!(format!("{}", DaemonMethod::SessionList), "session.list");
    }

    #[test]
    fn test_method_roundtrip() {
        let methods = [
            DaemonMethod::Prompt, DaemonMethod::SessionList, DaemonMethod::SessionShow,
            DaemonMethod::ConfigGet, DaemonMethod::ConfigSet, DaemonMethod::McpDiscover,
            DaemonMethod::McpServers, DaemonMethod::PluginsList, DaemonMethod::PluginsReload,
            DaemonMethod::Health,
        ];
        for method in &methods {
            let s = method.as_str();
            let parsed: DaemonMethod = s.parse().unwrap();
            assert_eq!(parsed, *method);
        }
    }

    // ─── Streaming Events ──────────────────────────────────────

    #[test]
    fn test_event_token_serialize() {
        let event = DaemonEvent::Token { content: "Hola mundo".into() };
        let json = event.to_json_line(1);
        assert!(json.contains("\"type\":\"token\""));
        assert!(json.contains("\"content\":\"Hola mundo\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_event_token_deserialize() {
        let json = r#"{"jsonrpc":"2.0","id":1,"result":{"type":"token","content":"Hola"}}"#;
        let resp: JsonRpcResponse = serde_json::from_str(json).unwrap();
        let result = resp.result.unwrap();
        let event: DaemonEvent = serde_json::from_value(result).unwrap();
        match event {
            DaemonEvent::Token { content } => assert_eq!(content, "Hola"),
            _ => panic!("Expected Token event"),
        }
    }

    #[test]
    fn test_event_tool_call_serialize() {
        let event = DaemonEvent::ToolCall {
            tool: "bash".into(),
            tool_params: serde_json::json!({"command": "apt update", "description": "Actualiza paquetes"}),
        };
        let json = event.to_json_line(1);
        assert!(json.contains("\"type\":\"tool_call\""));
        assert!(json.contains("\"tool\":\"bash\""));
        assert!(json.contains("\"command\":\"apt update\""));
    }

    #[test]
    fn test_event_tool_result_serialize() {
        let event = DaemonEvent::ToolResult {
            tool: "bash".into(),
            output: "Hit:1 http://archive.ubuntu.com".into(),
        };
        let json = event.to_json_line(2);
        assert!(json.contains("\"type\":\"tool_result\""));
        assert!(json.contains("\"tool\":\"bash\""));
    }

    #[test]
    fn test_event_done_serialize() {
        let event = DaemonEvent::Done { session_id: "uuid-123".into() };
        let json = event.to_json_line(1);
        assert!(json.contains("\"type\":\"done\""));
        assert!(json.contains("\"session_id\":\"uuid-123\""));
    }

    #[test]
    fn test_event_error_serialize() {
        let event = DaemonEvent::EventError { code: -32001, message: "Session not found".into() };
        let json = event.to_json_line(1);
        assert!(json.contains("\"type\":\"error\""));
        assert!(json.contains("\"code\":-32001"));
    }

    // ─── Parsing helpers ───────────────────────────────────────

    #[test]
    fn test_parse_valid_request() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"health"}"#;
        let req = parse_request(json).unwrap();
        assert_eq!(req.method, "health");
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not json at all";
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_method() {
        let json = r#"{"jsonrpc":"2.0","id":1}"#;
        let result = parse_request(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_line() {
        let buffer = "primera linea\nsegunda linea\ntercera";
        let (line, rest) = extract_line(buffer).unwrap();
        assert_eq!(line, "primera linea");
        assert_eq!(rest, "segunda linea\ntercera");
    }

    #[test]
    fn test_extract_line_no_newline() {
        let buffer = "sola linea";
        assert!(extract_line(buffer).is_none());
    }

    #[test]
    fn test_extract_lines() {
        let buffer = "line1\nline2\nline3\n";
        let (lines, remaining) = extract_lines(buffer);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "line1");
        assert_eq!(lines[1], "line2");
        assert_eq!(lines[2], "line3");
        assert!(remaining.is_empty());
    }

    #[test]
    fn test_extract_lines_empty() {
        let buffer = "";
        let (lines, remaining) = extract_lines(buffer);
        assert!(lines.is_empty());
        assert_eq!(remaining, "");
    }

    // ─── Error codes ───────────────────────────────────────────

    #[test]
    fn test_error_code_values() {
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::INVALID_REQUEST, -32600);
        assert_eq!(error_codes::METHOD_NOT_FOUND, -32601);
        assert_eq!(error_codes::INVALID_PARAMS, -32602);
        assert_eq!(error_codes::INTERNAL_ERROR, -32603);
        assert_eq!(error_codes::DAEMON_NOT_RUNNING, -32000);
        assert_eq!(error_codes::SESSION_NOT_FOUND, -32001);
    }

    // ─── Default implementations ───────────────────────────────

    #[test]
    fn test_prompt_flags_default() {
        let flags = PromptFlags::default();
        assert!(!flags.batch);
        assert!(!flags.reasoning);
        assert!(!flags.fast);
        assert!(!flags.dry_run);
        assert!(!flags.destructive);
    }

    // ─── Edge cases ────────────────────────────────────────────

    #[test]
    fn test_serialize_response_handles_error() {
        let resp = JsonRpcResponse::error(0, error_codes::INTERNAL_ERROR, "test error", None);
        let json = serialize_response(&resp);
        assert!(json.contains("test error"));
    }

    #[test]
    fn test_request_deserialize_with_extra_fields() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"prompt","extra":"ignored"}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.method, "prompt");
    }

    #[test]
    fn test_event_deserialize_unknown_type() {
        let json = r#"{"type":"unknown","data":"test"}"#;
        let result = serde_json::from_str::<DaemonEvent>(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_response_is_error_method() {
        let ok_resp = JsonRpcResponse::success(1, serde_json::json!({"ok": true}));
        assert!(!ok_resp.is_error());

        let err_resp = JsonRpcResponse::error(1, error_codes::INTERNAL_ERROR, "err", None);
        assert!(err_resp.is_error());
    }

    #[test]
    fn test_parse_method_from_request() {
        let req = JsonRpcRequest::new(1, "health", None);
        assert_eq!(req.parse_method(), Ok(DaemonMethod::Health));

        let req = JsonRpcRequest::new(1, "invalid_method", None);
        assert!(req.parse_method().is_err());
    }

    // ─── Line-based protocol (realistic scenario) ──────────────

    #[test]
    fn test_line_protocol_multiple_events() {
        let events = vec![
            DaemonEvent::Token { content: "Iniciando".into() },
            DaemonEvent::ToolCall {
                tool: "bash".into(),
                tool_params: serde_json::json!({"command": "ls"}),
            },
            DaemonEvent::Token { content: "Listo".into() },
            DaemonEvent::Done { session_id: "sess-1".into() },
        ];

        let mut buffer = String::new();
        for event in &events {
            buffer.push_str(&event.to_json_line(1));
            buffer.push('\n');
        }

        let (lines, _) = extract_lines(&buffer);
        assert_eq!(lines.len(), 4);

        for (i, line) in lines.iter().enumerate() {
            let resp: JsonRpcResponse = serde_json::from_str(line).unwrap();
            let result = resp.result.unwrap();
            let event: DaemonEvent = serde_json::from_value(result).unwrap();
            match (&event, &events[i]) {
                (DaemonEvent::Token { content: a }, DaemonEvent::Token { content: b }) => {
                    assert_eq!(a, b);
                }
                (DaemonEvent::ToolCall { tool: a, .. }, DaemonEvent::ToolCall { tool: b, .. }) => {
                    assert_eq!(a, b);
                }
                (DaemonEvent::Done { session_id: a }, DaemonEvent::Done { session_id: b }) => {
                    assert_eq!(a, b);
                }
                _ => panic!("Event mismatch at position {}", i),
            }
        }
    }
}
