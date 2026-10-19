use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

struct CdpRequest {
    id: u64,
    method: &'static str,
    params: Value,
    respond_to: oneshot::Sender<Result<Value, String>>,
}

#[derive(Clone)]
pub struct CdpClient {
    tx: mpsc::Sender<CdpRequest>,
    counter: Arc<AtomicU64>,
}

impl CdpClient {
    pub async fn connect(ws_url: &str) -> Result<Self, String> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url)
            .await
            .map_err(|e| format!("WebSocket connection failed ({ws_url}): {e}"))?;

        let (mut write_half, mut read_half) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<CdpRequest>(256);

        tokio::spawn(async move {
            let mut pending: HashMap<u64, oneshot::Sender<Result<Value, String>>> = HashMap::new();

            loop {
                tokio::select! {
                    Some(req) = rx.recv() => {
                        let payload = json!({
                            "id": req.id,
                            "method": req.method,
                            "params": req.params,
                        });

                        let text = match serde_json::to_string(&payload) {
                            Ok(t) => t,
                            Err(e) => {
                                let _ = req.respond_to.send(Err(e.to_string()));
                                continue;
                            }
                        };

                        pending.insert(req.id, req.respond_to);

                        if let Err(e) = write_half.send(Message::Text(text.into())).await {
                            if let Some(responder) = pending.remove(&req.id) {
                                let _ = responder.send(Err(format!("WebSocket write error: {e}")));
                            }
                            break;
                        }
                    }

                    frame = read_half.next() => {
                        match frame {
                            Some(Ok(Message::Text(raw))) => {
                                if let Ok(val) = serde_json::from_str::<Value>(&raw) {
                                    if let Some(id) = val.get("id").and_then(|v| v.as_u64()) {
                                        if let Some(responder) = pending.remove(&id) {
                                            if let Some(err) = val.get("error") {
                                                let msg = err.get("message")
                                                    .and_then(|m| m.as_str())
                                                    .unwrap_or("CDP error");
                                                let _ = responder.send(Err(msg.to_string()));
                                            } else {
                                                let res = val.get("result").cloned().unwrap_or(Value::Null);
                                                let _ = responder.send(Ok(res));
                                            }
                                        }
                                    }
                                }
                            }
                            Some(Ok(Message::Ping(data))) => {
                                let _ = write_half.send(Message::Pong(data)).await;
                            }
                            Some(Ok(Message::Close(_))) | None => break,
                            _ => {}
                        }
                    }
                }
            }

            for (_, responder) in pending.drain() {
                let _ = responder.send(Err("CDP connection closed".to_string()));
            }
        });

        Ok(Self {
            tx,
            counter: Arc::new(AtomicU64::new(1)),
        })
    }

    pub async fn call(&self, method: &'static str, params: Value) -> Result<Value, String> {
        let id = self.counter.fetch_add(1, Ordering::SeqCst);
        let (respond_to, rx) = oneshot::channel();

        self.tx
            .send(CdpRequest {
                id,
                method,
                params,
                respond_to,
            })
            .await
            .map_err(|_| "CDP channel closed".to_string())?;

        rx.await.map_err(|_| "CDP response dropped".to_string())?
    }

    pub async fn eval(&self, expression: &str) -> Result<Value, String> {
        let res = self
            .call(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
            )
            .await?;

        if let Some(exc) = res.get("exceptionDetails") {
            let msg = exc
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("Evaluation exception");
            return Err(format!("JavaScript exception: {msg}"));
        }

        Ok(res
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }

    pub async fn dispatch_char(&self, ch: char, dwell_ms: u64) -> Result<(), String> {
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "text": ch.to_string(),
            }),
        )
        .await?;

        if dwell_ms > 0 {
            tokio::time::sleep(Duration::from_millis(dwell_ms)).await;
        }

        self.call("Input.dispatchKeyEvent", json!({ "type": "keyUp" }))
            .await?;
        Ok(())
    }

    pub async fn dispatch_backspace(&self, dwell_ms: u64) -> Result<(), String> {
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": "Backspace",
                "code": "Backspace",
                "windowsVirtualKeyCode": 8,
            }),
        )
        .await?;

        if dwell_ms > 0 {
            tokio::time::sleep(Duration::from_millis(dwell_ms)).await;
        }

        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": "Backspace",
                "code": "Backspace",
                "windowsVirtualKeyCode": 8,
            }),
        )
        .await?;
        Ok(())
    }

    pub async fn dispatch_space(&self, dwell_ms: u64) -> Result<(), String> {
        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyDown",
                "key": " ",
                "code": "Space",
                "windowsVirtualKeyCode": 32,
                "text": " ",
            }),
        )
        .await?;

        if dwell_ms > 0 {
            tokio::time::sleep(Duration::from_millis(dwell_ms)).await;
        }

        self.call(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "key": " ",
                "code": "Space",
                "windowsVirtualKeyCode": 32,
            }),
        )
        .await?;
        Ok(())
    }
}
