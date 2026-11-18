//! Configuración leída del entorno (y de `.env`). No hace red.

use crate::realtime::Channel;

/// Ids manuales para saltarse la consulta a `kick.com/api/v2` si esta es bloqueada.
pub fn channel_override(slug: &str) -> Option<Channel> {
    load_dotenv();
    channel_override_from(slug, env_var)
}

fn channel_override_from(slug: &str, lookup: impl Fn(&str) -> Option<String>) -> Option<Channel> {
    Some(Channel {
        slug: slug.to_string(),
        id: lookup("KICK_CHANNEL_ID")?.parse().ok()?,
        chatroom_id: lookup("KICK_CHATROOM_ID")?.parse().ok()?,
    })
}

fn load_dotenv() {
    // Un .env ausente no es un error: las variables pueden venir del entorno.
    dotenvy::dotenv().ok();
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn lookup(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn ids_manuales_requieren_ambos_y_numericos() {
        let ok = channel_override_from(
            "x",
            lookup(&[("KICK_CHANNEL_ID", "7"), ("KICK_CHATROOM_ID", "9")]),
        );
        assert_eq!(
            ok,
            Some(Channel {
                slug: "x".into(),
                id: 7,
                chatroom_id: 9
            })
        );
        assert!(channel_override_from("x", lookup(&[("KICK_CHANNEL_ID", "7")])).is_none());
        assert!(
            channel_override_from(
                "x",
                lookup(&[("KICK_CHANNEL_ID", "a"), ("KICK_CHATROOM_ID", "9")])
            )
            .is_none()
        );
    }
}
