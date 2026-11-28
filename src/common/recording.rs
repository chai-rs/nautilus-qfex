use serde_json::{Map, Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use nautilus_core::time::get_atomic_clock_realtime;

/// A cloneable handle that hands record lines to the writer.
#[derive(Clone, Debug)]
pub struct QfexRecorder {
    tx: UnboundedSender<String>,
}

impl QfexRecorder {
    /// A recorder and the receiver of its lines, one JSON object a line.
    #[must_use]
    pub fn channel() -> (Self, UnboundedReceiver<String>) {
        let (tx, rx) = unbounded_channel();
        (Self { tx }, rx)
    }

    /// One REST answer: request method, endpoint and query, then status and raw body.
    pub fn rest(
        &self,
        method: &str,
        endpoint: &str,
        params: &[(&str, String)],
        status: u16,
        body: &[u8],
    ) {
        let params: Map<String, Value> = params
            .iter()
            .map(|(key, value)| ((*key).to_string(), Value::from(value.as_str())))
            .collect();
        self.push(json!({
            "dir": "rest",
            "method": method,
            "endpoint": endpoint,
            "params": params,
            "status": status,
            "body": String::from_utf8_lossy(body),
        }));
    }

    /// One frame sent on the trade socket. An `auth` frame becomes `{"type":"auth"}`.
    pub fn sent(&self, text: &str) {
        let is_auth =
            serde_json::from_str::<Value>(text).is_ok_and(|frame| frame["type"] == "auth");
        let text = if is_auth {
            json!({"type": "auth"}).to_string()
        } else {
            text.to_string()
        };
        self.push(json!({"dir": "sent", "text": text}));
    }

    /// One text frame received on the trade socket. An error that echoes an `auth` request
    /// loses the echoed `params`.
    pub fn received(&self, text: &str) {
        let text = match serde_json::from_str::<Value>(text) {
            Ok(mut frame) => {
                redact_echoed_auth(&mut frame);
                frame.to_string()
            }
            Err(_) => text.to_string(),
        };
        self.push(json!({"dir": "received", "text": text}));
    }

    fn push(&self, mut line: Value) {
        line["ts"] = Value::from(get_atomic_clock_realtime().get_time_ns().as_u64());
        let _ = self.tx.send(line.to_string());
    }
}

/// The docs put `incoming_message` at the top of an error frame or inside `err`.
fn redact_echoed_auth(frame: &mut Value) {
    redact_in(frame);
    if let Some(object) = frame.as_object_mut() {
        object.values_mut().for_each(redact_in);
    }
}

fn redact_in(holder: &mut Value) {
    if let Some(echoed) = holder
        .get_mut("incoming_message")
        .and_then(Value::as_object_mut)
        && echoed.get("type").and_then(Value::as_str) == Some("auth")
    {
        echoed.remove("params");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(record: impl FnOnce(&QfexRecorder)) -> Vec<Value> {
        let (recorder, mut rx) = QfexRecorder::channel();
        record(&recorder);
        drop(recorder);
        std::iter::from_fn(|| rx.try_recv().ok())
            .map(|line| serde_json::from_str(&line).unwrap())
            .collect()
    }

    #[test]
    fn sent_auth_keeps_only_its_type() {
        let auth = r#"{"type":"auth","params":{"hmac":{"public_key":"PK","nonce":"N","unix_ts":1,"signature":"SIG"}}}"#;
        let lines = lines(|recorder| recorder.sent(auth));
        assert_eq!(lines[0]["dir"], "sent");
        assert_eq!(lines[0]["text"], r#"{"type":"auth"}"#);
        assert!(lines[0]["ts"].as_u64().is_some());
    }

    #[test]
    fn other_frames_are_recorded_as_they_are() {
        let add = r#"{"type":"add_order","params":{"client_order_id":"O-1"}}"#;
        let ack = r#"{"order_response":{"status":"ACK"}}"#;
        let lines = lines(|recorder| {
            recorder.sent(add);
            recorder.received(ack);
            recorder.received("not json");
        });
        assert_eq!(lines[0]["text"], add);
        assert_eq!(lines[1]["dir"], "received");
        assert_eq!(lines[1]["text"], ack);
        assert_eq!(lines[2]["text"], "not json");
    }

    #[test]
    fn an_echoed_auth_loses_its_params_at_the_top_and_inside_err() {
        let top = r#"{"type":"error","incoming_message":{"type":"auth","params":{"hmac":{"signature":"SIG"}}}}"#;
        let nested = r#"{"err":{"message":"no","incoming_message":{"type":"auth","params":{"hmac":{"signature":"SIG"}}}}}"#;
        let order = r#"{"err":{"incoming_message":{"type":"add_order","params":{"client_order_id":"O-1"}}}}"#;
        let lines = lines(|recorder| {
            recorder.received(top);
            recorder.received(nested);
            recorder.received(order);
        });
        let text = |line: &Value| -> Value {
            serde_json::from_str(line["text"].as_str().unwrap()).unwrap()
        };
        assert_eq!(
            text(&lines[0]),
            json!({"type": "error", "incoming_message": {"type": "auth"}})
        );
        assert_eq!(
            text(&lines[1]),
            json!({"err": {"message": "no", "incoming_message": {"type": "auth"}}})
        );
        assert!(lines[2]["text"].as_str().unwrap().contains("O-1"));
        assert!(lines.iter().all(|line| !line.to_string().contains("SIG")));
    }

    #[test]
    fn rest_answer_carries_request_and_answer() {
        let params = [("api_version", "2026-10-29".to_string())];
        let lines = lines(|recorder| recorder.rest("GET", "/user/fees", &params, 401, b"{}"));
        assert_eq!(
            lines[0],
            json!({"dir": "rest", "method": "GET", "endpoint": "/user/fees",
                "params": {"api_version": "2026-10-29"}, "status": 401, "body": "{}",
                "ts": lines[0]["ts"]})
        );
    }
}
