use axum::extract::ws::{Message, WebSocketUpgrade};
use axum::response::IntoResponse;

pub(super) async fn stream(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        let _ = socket
            .send(Message::Text(
                "{\"type\":\"hello\",\"service\":\"ton-l2-rollup\"}".to_owned(),
            ))
            .await;
    })
}
