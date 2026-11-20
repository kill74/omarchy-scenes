//! Phase 1 acceptance: the event path is tested against a mock Hyprland
//! socket2 only — no live compositor is needed.

use std::path::PathBuf;

use scenes_core::events::{self, HyprEvent};
use scenes_core::signal::{Context, Signal};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!("scenes-mock-socket-{}.sock", std::process::id()))
}

#[tokio::test]
async fn streams_and_parses_socket2_events() {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).unwrap_or_else(|e| panic!("{e}"));

    let writer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap_or_else(|e| panic!("{e}"));
        stream
            .write_all(b"activewindow>>code,main.rs\nworkspacev2>>1,1\n")
            .await
            .unwrap_or_else(|e| panic!("{e}"));
    });

    let stream = UnixStream::connect(&path)
        .await
        .unwrap_or_else(|e| panic!("{e}"));
    let mut lines = BufReader::new(stream).lines();
    let mut ctx = Context::default();

    let first = lines
        .next_line()
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_default();
    let event = events::parse(&first);
    assert_eq!(
        event,
        HyprEvent::ActiveWindow {
            class: "code".into(),
            title: "main.rs".into()
        }
    );
    ctx.apply(&Signal::Hypr(event));

    let second = lines
        .next_line()
        .await
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_default();
    ctx.apply(&Signal::Hypr(events::parse(&second)));

    assert_eq!(ctx.window_class, "code");
    assert_eq!(ctx.workspace, "1");
    let _ = writer.await;
    let _ = std::fs::remove_file(path);
}
