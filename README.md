# Kick Chat Listener en Rust

Proyecto de aprendizaje para conectarse al chat público de un canal de Kick y mostrar los mensajes en la terminal. La idea es entender el flujo completo de descubrimiento del canal, autenticación temporal, conexión WebSocket y lectura de eventos, de modo que pueda recrearse después en otro lenguaje o integrarse en otra aplicación.

## Qué hace

Al ejecutar el programa con el nombre de un canal, este:

1. Consulta la información pública del canal y obtiene el ID del canal y del chatroom.
2. Genera un ID de cliente y solicita a Kick un token temporal de invitado.
3. Negocia el proveedor y la URL del servicio realtime.
4. Abre un WebSocket, envía el token y se suscribe a `chatrooms.<chatroom_id>.v2`.
5. Responde a los pings y muestra los eventos `App\Events\ChatMessageEvent` como `usuario: mensaje`.

El programa solo imprime mensajes de chat. Otros eventos que lleguen por el feed se ignoran.

## Requisitos y uso

Necesitas tener instalado Rust y Cargo. Desde la raíz del proyecto, ejecuta:

```bash
cargo run -- <nombre-del-canal>
```

Por ejemplo:

```bash
cargo run -- xqc
```

Pulsa `Ctrl+C` para detener el listener. La primera ejecución necesita conexión a Internet para descargar las dependencias de Cargo.

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

- `tokio`: ejecutor asíncrono para las operaciones de red.
- `reqwest`: peticiones HTTP para resolver el canal y negociar la conexión.
- `tokio-tungstenite` y `futures-util`: conexión WebSocket y lectura/escritura de frames.
- `serde_json`: construcción y lectura de JSON.
- `uuid`: creación del ID único del cliente.
- `anyhow`: propagación legible de errores.

## Estructura

```text
Cargo.toml   # dependencias y configuración del paquete
src/main.rs  # flujo HTTP, WebSocket y presentación de mensajes
```

## Alcance y notas

La conexión realtime y los endpoints `kick.com/api/v2` y `web.kick.com/api/v1/realtime` usados aquí no forman parte de un contrato público estable. Kick puede cambiar las rutas, los campos o el protocolo y romper el ejemplo. La conexión de invitado también es temporal; esta primera versión no renueva el token ni reconecta automáticamente si se cae el socket.

Para una aplicación más completa, los siguientes pasos serían añadir reconexión con espera progresiva, renovación del token, tipos Rust para los eventos y manejo de otros eventos del chat. La documentación técnica comunitaria del protocolo está en [kicksdk/docs/KICK.md](https://github.com/AnisAbdellatif/kicksdk/blob/main/docs/KICK.md); debe tratarse como referencia de terceros y contrastarse con el comportamiento actual de Kick.
