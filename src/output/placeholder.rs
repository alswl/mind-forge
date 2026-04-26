use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PlaceholderInvocation {
    pub command: String,
    pub args: Value,
}

#[derive(Debug, Serialize)]
pub struct PlaceholderEnvelope<'a> {
    pub status: &'static str,
    pub command: &'a str,
    pub data: Option<Value>,
    pub args: &'a Value,
    pub message: &'static str,
}

impl PlaceholderInvocation {
    pub fn new(command: impl Into<String>, args: Value) -> Self {
        Self { command: command.into(), args }
    }

    pub fn to_json(&self) -> PlaceholderEnvelope<'_> {
        PlaceholderEnvelope {
            status: "not_implemented",
            command: &self.command,
            data: None,
            args: &self.args,
            message: "This command is a framework placeholder; implementation will follow.",
        }
    }

    pub fn args_text(&self) -> String {
        match &self.args {
            Value::Object(map) => {
                let mut pairs = map
                    .iter()
                    .map(|(key, value)| format!("{key}={}", value_to_text(value)))
                    .collect::<Vec<_>>();
                pairs.sort();
                pairs.join(", ")
            }
            value => value_to_text(value),
        }
    }
}

fn value_to_text(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(values) => {
            let items = values.iter().map(value_to_text).collect::<Vec<_>>().join("|");
            format!("[{items}]")
        }
        Value::Object(_) => value.to_string(),
    }
}
