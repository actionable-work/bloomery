use axum::{
    extract::Path,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct StatusResponse {
    pub status: String,
    pub service: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct GreetResponse {
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct EchoPayload {
    pub text: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct EchoResponse {
    pub echo: String,
}

async fn health_check() -> Json<StatusResponse> {
    Json(StatusResponse {
        status: "ok".to_string(),
        service: "axum-server".to_string(),
    })
}

async fn greet(Path(name): Path<String>) -> Json<GreetResponse> {
    Json(GreetResponse {
        message: format!("Hello, {}!", name),
    })
}

async fn echo(Json(payload): Json<EchoPayload>) -> Json<EchoResponse> {
    Json(EchoResponse {
        echo: payload.text,
    })
}

pub fn app() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/greet/:name", get(greet))
        .route("/echo", post(echo))
}

#[tokio::main]
async fn main() {
    let app = app();
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");

    axum::serve(listener, app)
        .await
        .expect("server error");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_types() {
        let status = StatusResponse {
            status: "ok".to_string(),
            service: "axum-server".to_string(),
        };
        let serialized = serde_json::to_string(&status).unwrap();
        let deserialized: StatusResponse = serde_json::from_str(&serialized).unwrap();
        assert_eq!(status, deserialized);
    }
}
