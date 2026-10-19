use super::super::outbound::Command;
use super::*;
use crate::app::module::common::streaming::commands::StreamingSendFrameKind;
use crate::app::module::common::streaming::websocket::{socket::BudgetSocket, websocket_config};
use std::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::{protocol::Role, WebSocket};

fn pair() -> (WireSocket, WebSocket<TcpStream>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let server = listener.accept().unwrap().0;
    server.set_nonblocking(true).unwrap();
    client.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    (
        WebSocket::from_raw_socket(BudgetSocket::new(server), Role::Server, Some(websocket_config())),
        WebSocket::from_raw_socket(client, Role::Client, None),
    )
}

#[test]
fn original_recipient_cannot_retarget_a_replacement_with_the_same_public_label() {
    let count = Arc::new(AtomicUsize::new(0));
    let mut peers = Peers::new(count);
    let (socket, _old_peer) = pair();
    peers.add(socket, "same-label".into(), "old".into());
    let mut broadcast = Broadcast::new(
        Command::new(StreamingSendFrameKind::Text, b"old command").unwrap(),
        peers.entries.iter().map(|peer| peer.id),
    )
    .unwrap();
    peers.remove(0, None);
    let (socket, mut replacement) = pair();
    peers.add(socket, "same-label".into(), "replacement".into());
    assert!(
        matches!(peers.write(&mut broadcast, 0), Some(Event::Error(message)) if message.contains("will not be replayed"))
    );
    assert_eq!(broadcast.next_recipient(), None);
    let mut next = Broadcast::new(
        Command::new(StreamingSendFrameKind::Text, b"new command").unwrap(),
        peers.entries.iter().map(|peer| peer.id),
    )
    .unwrap();
    assert!(peers.write(&mut next, 0).is_none());
    assert_eq!(replacement.read().unwrap(), Message::text("new command"));
}
