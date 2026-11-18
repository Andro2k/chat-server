use anyhow::{Context, Result, anyhow};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};
use uuid::Uuid;

const KICK_API: &str = "https://kick.com/api/v2/channels";
const REALTIME_API: &str = "https://web.kick.com/api/v1/realtime";

#[tokio::main]
async fn main() -> Result<()> {
    let Some(channel_slug) = std::env::args().nth(1) else {
        eprintln!("Uso: cargo run -- <nombre-del-canal>");
        eprintln!("Ejemplo: cargo run -- xqc");
        std::process::exit(2);
    };

    let http = reqwest::Client::builder()
        .user_agent("chat-server/0.1 (Rust learning project)")
        .build()?;

    println!("Buscando el chat de Kick/{channel_slug}...");
    let channel_url = format!("{KICK_API}/{channel_slug}");
    let channel: Value = http
        .get(channel_url)
        .send()
        .await?
        .error_for_status()
        .context("Kick no pudo encontrar ese canal")?
        .json()
        .await?;

    let channel_id = channel["id"]
        .as_u64()
        .ok_or_else(|| anyhow!("No encontré el id del canal en la respuesta de Kick"))?;
    let chatroom_id = channel["chatroom"]["id"]
        .as_u64()
        .ok_or_else(|| anyhow!("No encontré el id del chatroom en la respuesta de Kick"))?;

    // Kick entrega una identidad temporal de invitado para conectarse al feed.
    let client_id = Uuid::new_v4().to_string();
    let token_response: Value = http
        .post(format!("{REALTIME_API}/auth/connection"))
        .json(&json!({ "client_id": client_id }))
        .send()
        .await?
        .error_for_status()
        .context("No se pudo pedir el token temporal de Kick")?
        .json()
        .await?;
    let token = token_response["data"]["token"]
        .as_str()
        .ok_or_else(|| anyhow!("Kick no incluyó un token en su respuesta"))?;

    // La URL del servidor realtime se negocia para cada conexión.
    let connection_response: Value = http
        .post(format!(
            "{REALTIME_API}/channels/{channel_id}/chat/connection"
        ))
        .json(&json!({
            "client": { "id": client_id, "type": "web" },
            "capabilities": {
                "accepted_providers": [
                    { "provider": "centrifugo" },
                    { "provider": "pusher" }
                ]
            }
        }))
        .send()
        .await?
        .error_for_status()
        .context("No se pudo negociar la conexión realtime con Kick")?
        .json()
        .await?;
    let websocket_url = connection_response["data"]["connections"]
        .as_array()
        .and_then(|connections| {
            connections
                .iter()
                .find(|connection| connection["provider"] == "centrifugo")
        })
        .and_then(|connection| connection["credentials"]["url"].as_str())
        .ok_or_else(|| anyhow!("Kick no ofreció una conexión Centrifugo"))?;

    let mut request = websocket_url.into_client_request()?;
    request.headers_mut().insert(
        "Origin",
        "https://kick.com".parse().expect("Origin fijo válido"),
    );
    let (mut socket, _) = connect_async(request)
        .await
        .context("No se pudo abrir el WebSocket de Kick")?;

    // Centrifugo espera primero el token y luego una suscripción al chatroom.
    socket
        .send(Message::Text(
            json!({ "id": 1, "connect": { "token": token, "name": "rust-chat" } })
                .to_string()
                .into(),
        ))
        .await?;
    socket
        .send(Message::Text(
            json!({
                "id": 2,
                "subscribe": { "channel": format!("chatrooms.{chatroom_id}.v2") }
            })
            .to_string()
            .into(),
        ))
        .await?;

    println!("Conectado a Kick/{channel_slug}. Esperando mensajes... (Ctrl+C para salir)\n");

    while let Some(frame) = socket.next().await {
        match frame? {
            Message::Text(text) => {
                for line in text.lines() {
                    let Ok(frame) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };

                    // Kick manda un objeto vacío como ping; responderlo mantiene viva la sesión.
                    if frame.as_object().is_some_and(|object| object.is_empty()) {
                        socket.send(Message::Text("{}".into())).await?;
                        continue;
                    }

                    if let Some(publication) = frame.get("push").and_then(|push| push.get("pub")) {
                        show_chat_message(publication);
                    }
                }
            }
            Message::Ping(bytes) => socket.send(Message::Pong(bytes)).await?,
            Message::Close(_) => break,
            _ => {}
        }
    }

    println!("La conexión con Kick se cerró.");
    Ok(())
}

fn show_chat_message(publication: &Value) {
    let Some(event) = publication["data"]["event"].as_str() else {
        return;
    };
    if event != "App\\Events\\ChatMessageEvent" {
        return;
    }

    // El evento interno también puede venir como JSON serializado dentro del JSON exterior.
    let event_data = match &publication["data"]["data"] {
        Value::String(serialized) => serde_json::from_str::<Value>(serialized).ok(),
        Value::Object(_) => Some(publication["data"]["data"].clone()),
        _ => None,
    };
    let Some(message) = event_data else {
        return;
    };
    let username = message["sender"]["username"].as_str().unwrap_or("alguien");
    let content = message["content"].as_str().unwrap_or("");
    println!("{username}: {content}");
}
