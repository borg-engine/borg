use std::{collections::VecDeque, sync::Arc};

use tokio::sync::broadcast;

pub(crate) struct BroadcastLayer {
    pub tx: broadcast::Sender<String>,
    pub ring: Arc<std::sync::Mutex<VecDeque<String>>>,
}

struct MessageVisitor<'a> {
    message: &'a mut String,
    metadata: &'a mut serde_json::Map<String, serde_json::Value>,
}

impl tracing::field::Visit for MessageVisitor<'_> {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else {
            self.metadata.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.message.clear();
            use std::fmt::Write;
            let _ = write!(self.message, "{value:?}");
            // Strip surrounding quotes added by Debug on &str
            if self.message.starts_with('"') && self.message.ends_with('"') {
                *self.message = self.message[1..self.message.len() - 1].to_string();
            }
            return;
        }
        let mut rendered = String::new();
        use std::fmt::Write;
        let _ = write!(rendered, "{value:?}");
        if rendered.starts_with('"') && rendered.ends_with('"') {
            rendered = rendered[1..rendered.len() - 1].to_string();
        }
        self.metadata.insert(
            field.name().to_string(),
            serde_json::Value::String(rendered),
        );
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else {
            self.metadata
                .insert(field.name().to_string(), serde_json::Value::from(value));
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else {
            self.metadata
                .insert(field.name().to_string(), serde_json::Value::from(value));
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else {
            self.metadata
                .insert(field.name().to_string(), serde_json::Value::from(value));
        }
    }

    fn record_f64(&mut self, field: &tracing::field::Field, value: f64) {
        if field.name() == "message" {
            *self.message = value.to_string();
        } else if let Some(number) = serde_json::Number::from_f64(value) {
            self.metadata
                .insert(field.name().to_string(), serde_json::Value::Number(number));
        }
    }
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for BroadcastLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let level = match *event.metadata().level() {
            tracing::Level::ERROR => "err",
            tracing::Level::WARN => "warn",
            tracing::Level::INFO => "info",
            tracing::Level::DEBUG => "debug",
            tracing::Level::TRACE => return,
        };

        let target = event.metadata().target();
        let category = if target.contains("http") {
            "http"
        } else if target.contains("search") || target.contains("vespa") {
            "search"
        } else if target.contains("chat") {
            "chat"
        } else if target.contains("upload")
            || target.contains("ingestion")
            || target.contains("storage")
            || target.contains("knowledge")
        {
            "storage"
        } else if target.contains("auth") {
            "auth"
        } else if target.contains("project") {
            "project"
        } else if target.contains("task") {
            "task"
        } else if target.contains("pipeline") {
            "pipeline"
        } else if target.contains("agent") || target.contains("claude") || target.contains("codex")
        {
            "agent"
        } else {
            "system"
        };

        let mut message = String::new();
        let mut metadata = serde_json::Map::new();
        event.record(&mut MessageVisitor {
            message: &mut message,
            metadata: &mut metadata,
        });

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut payload = serde_json::json!({
            "ts": ts,
            "level": level,
            "message": message,
            "category": category,
        });
        if !metadata.is_empty() {
            payload["metadata"] =
                serde_json::Value::String(serde_json::Value::Object(metadata).to_string());
        }
        let json = payload.to_string();

        let _ = self.tx.send(json.clone());
        if let Ok(mut ring) = self.ring.lock() {
            ring.push_back(json);
            if ring.len() > 500 {
                ring.pop_front();
            }
        }
    }
}
