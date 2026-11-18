# Kick Chat Listener en Rust

Prototipo de consola para conectarse a Kick: escucha el chat y los eventos del canal por WebSocket y, con tus credenciales OAuth del archivo `.env`, usa la API oficial de Kick (quién eres, estado del canal y envío de mensajes al chat). Sirve para entender el flujo completo y recrearlo después en otro lenguaje o dentro de otra aplicación.

## Qué hace

1. Resuelve el canal (ids del canal y del chatroom), pide un token temporal de invitado y negocia el servidor realtime (Centrifugo).
2. Abre el WebSocket y se suscribe a cuatro feeds: `chatrooms.<chatroom>.v2`, `chatroom_<chatroom>`, `channel.<canal>` y `channel_<canal>`.
3. Muestra cada evento en consola con hora, etiqueta y color. Si la conexión se cae, reconecta con espera creciente (1 s hasta 30 s).
4. Con `--login`, inicia sesión por OAuth 2.1 (PKCE), muestra el usuario autenticado y envía al chat lo que escribas en la consola.

## Uso

Necesitas Rust y Cargo (edición 2024, Rust 1.85 o superior).

```bash
cargo run -- <canal>            # solo lectura, sin credenciales
cargo run -- <canal> --raw      # además imprime el JSON original de cada evento
cargo run -- <canal> --login    # lectura + sesión OAuth + enviar mensajes escribiendo en la consola
cargo run -- --login            # solo inicia sesión y guarda los tokens
```

`Ctrl+C` detiene el programa. Si defines la variable `NO_COLOR`, se desactivan los colores.

## Credenciales OAuth (`.env`)

1. Crea una app en <https://kick.com/settings/developer> y registra la Redirect URI (por defecto `http://localhost:8080/auth/callback`; debe apuntar a `localhost` o `127.0.0.1`).
2. Copia `.env.example` como `.env` y completa `KICK_CLIENT_ID` y `KICK_CLIENT_SECRET`.
3. Ejecuta `cargo run -- --login`. El programa imprime un enlace: ábrelo en el navegador, autoriza y vuelve a la consola.

| Variable | Obligatoria | Por defecto |
|---|---|---|
| `KICK_CLIENT_ID`, `KICK_CLIENT_SECRET` | con `--login` | n/a |
| `KICK_REDIRECT_URI` | no | `http://localhost:8080/auth/callback` |
| `KICK_SCOPES` | no | `user:read channel:read chat:write` |
| `KICK_TOKEN_FILE` | no | `.kick_tokens.json` |
| `KICK_CHANNEL_ID`, `KICK_CHATROOM_ID` | no | consulta a kick.com; úsalas si te devuelve 403 |

Los scopes pedidos deben estar habilitados en tu app de Kick; si no, Kick rechaza la autorización con `invalid_scope`. Los tokens se guardan en `.kick_tokens.json` (permisos 0600 en Unix), se renuevan solos y, si cambias `KICK_SCOPES`, se vuelve a pedir el login. `.env` y `.kick_tokens.json` están en `.gitignore`: no los subas al repositorio.

## Eventos que se muestran

| Etiqueta | Evento de Kick | Feed |
|---|---|---|
| `CHAT` | `ChatMessageEvent` | `chatrooms.<id>.v2` |
| `DEL`, `BAN` | `MessageDeletedEvent`, `UserBannedEvent` | `chatrooms.<id>.v2` |
| `SUB` | `SubscriptionEvent` | `chatrooms.<id>.v2` |
| `GIFT`, `REWARD` | `GiftedSubscriptionsEvent`, `RewardRedeemedEvent` | `chatroom_<id>` |
| `FOLLOW`, `LIVE`, `OFFLINE` | `FollowersUpdated`, `StreamerIsLive`, `StopStreamBroadcast` | `channel.<id>` |
| `OTHER` | cualquier evento sin interpretar (muestra nombre y JSON recortado) | cualquiera |

Los nombres de eventos y campos corresponden al esquema conocido del feed no oficial y no se han podido contrastar en vivo. Si un evento no se reconoce o le falta algún campo esperado, aparece como `OTHER`; usa `--raw` para ver su JSON y ajustar `src/events.rs`.

## Cómo recrear el flujo

La implementación actual está en `src/main.rs`. Para portar la idea a otro proyecto, conserva este orden y usa un cliente HTTP, un cliente WebSocket y un parser JSON equivalentes:

1. **Resolver el canal:** `GET https://kick.com/api/v2/channels/<slug>`. De la respuesta se leen `id` y `chatroom.id`.
2. **Obtener token de invitado:** `POST https://web.kick.com/api/v1/realtime/auth/connection` con `{"client_id":"<uuid>"}`. Guardar `data.token`.
3. **Negociar el transporte:** `POST https://web.kick.com/api/v1/realtime/channels/<channel_id>/chat/connection`, usando el mismo UUID en `client.id`, `client.type: "web"` y los proveedores aceptados. La respuesta contiene la URL WebSocket en `data.connections[].credentials.url` para el proveedor `centrifugo`.
4. **Abrir el WebSocket:** enviar el encabezado `Origin: https://kick.com` durante el handshake.
5. **Conectar y suscribirse:** enviar los comandos JSON de Centrifugo:

   ```json
   {"id":1,"connect":{"token":"<token>","name":"rust-chat"}}
   {"id":2,"subscribe":{"channel":"chatrooms.<chatroom_id>.v2"}}
   ```

6. **Procesar el feed:** leer los mensajes WebSocket, responder con `{}` cuando llegue el ping JSON vacío, buscar publicaciones en `push.pub` y filtrar el evento `App\Events\ChatMessageEvent`. El dato del evento puede ser un objeto JSON o una cadena que contiene JSON; en ambos casos, el nombre y texto están en `sender.username` y `content`.

## Dependencias Rust

- `tokio`: ejecutor asíncrono, temporizadores, señales y servidor del callback OAuth.
- `reqwest`: peticiones HTTP (canal, negociación realtime, token OAuth y API pública).
- `tokio-tungstenite` y `futures-util`: conexión WebSocket y lectura/escritura de frames.
- `serde` y `serde_json`: JSON de las respuestas y de los tokens guardados.
- `sha2`, `base64` y `rand`: verificador y desafío PKCE (S256) y valor `state`.
- `dotenvy`: carga del archivo `.env`.
- `chrono`: hora local en la consola.
- `uuid`: identificador del cliente realtime.
- `anyhow`: propagación legible de errores.

## Estructura

```text
src/main.rs       # argumentos y orquestación
src/config.rs     # variables de entorno y .env
src/realtime.rs   # WebSocket Centrifugo: conexión, feeds, reconexión
src/events.rs     # interpretación de eventos del feed
src/console.rs    # formato y colores de la salida
src/oauth.rs      # OAuth 2.1 + PKCE, callback local, almacén y renovación de tokens
src/api.rs        # API pública de Kick (users, channels, chat)
src/input.rs      # líneas de la consola -> mensajes de chat
docs/walkthroughs # documentación de cada cambio
```

Las pruebas (`cargo test`) cubren el parseo de eventos y frames, el backoff, PKCE (con el vector de la RFC 7636), el callback, el almacén de tokens y la lectura de la configuración. No hay pruebas contra servidores reales de Kick.

## Alcance y notas

La conexión realtime y los endpoints `kick.com/api/v2` y `web.kick.com/api/v1/realtime` no forman parte de un contrato público estable: Kick puede cambiarlos y romper el prototipo. La API oficial (`api.kick.com/public/v1` y OAuth en `id.kick.com`) sí está documentada, pero sus eventos oficiales se entregan por webhooks, que requieren una URL HTTPS pública y no se usan aquí; por eso los eventos se leen del feed WebSocket no oficial (solo lectura) y la API con OAuth se usa para actuar.

El token de invitado del feed no se renueva: al caerse o vencer la conexión, el programa reconecta con un token y un identificador nuevos. La documentación técnica comunitaria del protocolo está en [kicksdk/docs/KICK.md](https://github.com/AnisAbdellatif/kicksdk/blob/main/docs/KICK.md); debe tratarse como referencia de terceros y contrastarse con el comportamiento actual de Kick.
