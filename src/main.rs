mod console;
mod events;
mod realtime;

use anyhow::{Context, Result};
use tokio::sync::mpsc;

use console::Console;
use realtime::FeedItem;

const USAGE: &str = "Uso: cargo run -- <nombre-del-canal> [--raw]\n\
Ejemplo: cargo run -- xqc\n\n\
  --raw   muestra también el JSON original de cada evento";

#[derive(Debug, PartialEq)]
struct Cli {
    channel: String,
    raw: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut channel = None;
    let mut raw = false;
    for arg in args {
        match arg.as_str() {
            "--raw" => raw = true,
            flag if flag.starts_with("--") => return Err(format!("Opción desconocida: {flag}")),
            _ if channel.is_none() => channel = Some(arg),
            _ => return Err(format!("Argumento inesperado: {arg}")),
        }
    }
    let channel = channel.ok_or_else(|| "Falta el nombre del canal".to_string())?;
    Ok(Cli { channel, raw })
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = match parse_args(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            std::process::exit(2);
        }
    };
    let console = Console::new(cli.raw);

    let http = reqwest::Client::builder()
        .user_agent("chat-server/0.1 (Rust learning project)")
        .build()?;

    console.status(&format!("Buscando el chat de Kick/{}...", cli.channel));
    let channel = realtime::resolve_channel(&http, &cli.channel)
        .await
        .context("No se pudo resolver el canal")?;

    let (tx, mut rx) = mpsc::channel(256);
    tokio::spawn(realtime::run(http, channel, tx));
    console.status("Escuchando (Ctrl+C para salir)");

    loop {
        tokio::select! {
            item = rx.recv() => match item {
                Some(FeedItem::Event { feed, published }) => console.event(&feed, &published),
                Some(FeedItem::Status(text)) => console.status(&text),
                None => break,
            },
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    console.status("Hasta luego.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> impl Iterator<Item = String> {
        list.iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    #[test]
    fn parsea_canal_y_raw_en_cualquier_orden() {
        let esperado = Cli {
            channel: "xqc".into(),
            raw: true,
        };
        assert_eq!(
            parse_args(args(&["xqc", "--raw"])),
            Ok(Cli {
                channel: "xqc".into(),
                raw: true
            })
        );
        assert_eq!(parse_args(args(&["--raw", "xqc"])), Ok(esperado));
        assert_eq!(
            parse_args(args(&["xqc"])),
            Ok(Cli {
                channel: "xqc".into(),
                raw: false
            })
        );
    }

    #[test]
    fn rechaza_entradas_invalidas() {
        assert!(parse_args(args(&[])).is_err());
        assert!(parse_args(args(&["a", "b"])).is_err());
        assert!(parse_args(args(&["a", "--nope"])).is_err());
    }
}
