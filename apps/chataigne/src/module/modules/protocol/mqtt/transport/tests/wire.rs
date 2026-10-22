use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

pub(super) fn packet(stream: &mut TcpStream) -> (u8, Vec<u8>) {
    let mut byte = [0];
    stream.read_exact(&mut byte).unwrap();
    let kind = byte[0];
    let mut length = 0;
    let mut factor = 1;
    for _ in 0..4 {
        stream.read_exact(&mut byte).unwrap();
        length += usize::from(byte[0] & 127) * factor;
        if byte[0] & 128 == 0 {
            break;
        }
        factor *= 128;
    }
    assert!(length <= MAX_PACKET);
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload).unwrap();
    (kind, payload)
}

#[test]
fn real_rumqttc_publish_receipt_waits_for_the_loopback_broker_puback() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let mut cfg = config();
    cfg.remote_port = listener.local_addr().unwrap().port();
    let (sent, published) = mpsc::sync_channel(1);
    let (release, ack_gate) = mpsc::sync_channel(1);
    let broker = std::thread::spawn(move || {
        let mut connection = None;
        until(|| match listener.accept() {
            Ok((stream, _)) => {
                connection = Some(stream);
                true
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
            Err(error) => panic!("{error}"),
        });
        let mut connection = connection.unwrap();
        connection.set_nonblocking(false).unwrap();
        connection.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        assert_eq!(packet(&mut connection).0, 0x10);
        connection.write_all(&[0x20, 0x02, 0x00, 0x00]).unwrap();
        let (kind, bytes) = packet(&mut connection);
        assert_eq!(kind & 0xf0, 0x30);
        let topic = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        let id = &bytes[2 + topic..4 + topic];
        sent.send(()).unwrap();
        ack_gate.recv_timeout(Duration::from_secs(3)).unwrap();
        connection.write_all(&[0x40, 0x02, id[0], id[1]]).unwrap();
        std::thread::sleep(Duration::from_millis(50));
    });
    let lifetimes = slots();
    let mut handle = MqttTransportHandle::spawn_using(
        cfg,
        NodeId(1),
        notify(),
        &lifetimes,
        pool(16),
        pool(16),
        worker::NativeBroker::open,
    )
    .unwrap();
    until(|| handle.connected.load(Ordering::Acquire));
    let receipt = handle.send(request(), false).unwrap();
    published.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(receipt.status(), PublishStatus::Pending);
    release.send(()).unwrap();
    until(|| receipt.status() == PublishStatus::Acknowledged);
    handle.stop();
    drop(handle);
    broker.join().unwrap();
    assert!(lifetimes.wait_for_idle(Duration::from_secs(1)));
}
