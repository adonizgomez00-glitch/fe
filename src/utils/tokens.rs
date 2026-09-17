use serde_json::Value;

pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() as f64 / 4.0).ceil() as u32
}

pub fn estimate_messages_tokens(messages: &[crate::agent::llm::Message]) -> u32 {
    messages.iter().map(|m| {
        let role_tokens = estimate_tokens(&m.role);
        let content_tokens = estimate_tokens(&m.content);
        let tool_tokens = m.tool_calls.as_ref()
            .map(|tc| serde_json::to_string(tc).ok()
                .map(|s| estimate_tokens(&s))
                .unwrap_or(0))
            .unwrap_or(0);
        role_tokens + content_tokens + tool_tokens + 4
    }).sum()
}

pub fn estimate_schemas_tokens(schemas: &[Value]) -> u32 {
    estimate_tokens(&serde_json::to_string(schemas).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::llm::Message;

    #[test]
    fn test_estimate_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn test_estimate_short() {
        assert_eq!(estimate_tokens("hola"), 1);
    }

    #[test]
    fn test_estimate_exact_4() {
        assert_eq!(estimate_tokens("1234"), 1);
    }

    #[test]
    fn test_estimate_5_chars() {
        assert_eq!(estimate_tokens("12345"), 2);
    }

    #[test]
    fn test_estimate_messages_empty() {
        assert_eq!(estimate_messages_tokens(&[]), 0);
    }

    #[test]
    fn test_estimate_messages_single() {
        let msgs = vec![Message {
            role: "user".into(),
            content: "hola".into(),
            tool_calls: None,
        }];
        let tokens = estimate_messages_tokens(&msgs);
        assert!(tokens > 0);
    }

    #[test]
    fn test_estimate_schemas_empty() {
        assert_eq!(estimate_schemas_tokens(&[]), 1);
    }

    #[test]
    fn test_estimate_schemas_non_empty() {
        let schemas = vec![serde_json::json!({"type": "object"})];
        assert!(estimate_schemas_tokens(&schemas) > 0);
    }

    #[test]
    fn test_estimate_messages_with_tool_calls() {
        let msgs = vec![Message {
            role: "assistant".into(),
            content: "".into(),
            tool_calls: Some(vec![crate::agent::llm::ToolCall {
                call_type: None,
                function: crate::agent::llm::ToolFunction {
                    name: "bash".into(),
                    arguments: serde_json::json!({"command": "ls"}),
                },
            }]),
        }];
        let tokens = estimate_messages_tokens(&msgs);
        assert!(tokens > 4);
    }
}
