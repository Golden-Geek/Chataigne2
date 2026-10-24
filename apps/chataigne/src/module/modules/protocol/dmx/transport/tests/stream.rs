use super::*;
use sacn::{Cid, Packet, packet::Payload};

fn data(socket: &UdpSocket, protocol: DmxProtocol) -> (u8, Vec<u8>, bool) {
    let mut bytes = [0; 2048];
    let (length, _) = socket.recv_from(&mut bytes).unwrap();
    match protocol {
        DmxProtocol::ArtNet => {
            let ArtCommand::Output(output) = ArtCommand::from_buffer(&bytes[..length]).unwrap() else {
                panic!("expected ArtDMX");
            };
            (output.sequence, output.data.as_ref().to_vec(), false)
        }
        DmxProtocol::Sacn => {
            let packet = Packet::parse(&bytes[..length]).unwrap();
            assert_eq!(packet.cid, Cid::from_bytes([77; 16]));
            let Payload::Data(data) = packet.payload else {
                panic!("unicast endpoint emitted non-data traffic");
            };
            (data.sequence_number.get(), data.values.to_vec(), data.stream_terminated)
        }
    }
}

#[test]
fn native_worker_clocks_fifo_blackout_and_refresh_then_terminates_on_the_unicast_route() {
    for protocol in [DmxProtocol::ArtNet, DmxProtocol::Sacn] {
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut transport = spawn(DmxTransportConfig {
            protocol,
            source_id: [77; 16],
            bind_ip: Ipv4Addr::LOCALHOST.into(),
            listen_port: protocol.default_port(),
            receive_enabled: false,
            universe: 1,
            destination: Some(receiver.local_addr().unwrap()),
        });
        let receipts: Vec<_> = [11, 22, 0]
            .into_iter()
            .map(|value| transport.send(DmxFrame::new(1, vec![value; 512]).unwrap()).unwrap())
            .collect();
        let mut last_sequence = None;
        for expected in [11, 22, 0] {
            let (sequence, levels, terminated) = data(&receiver, protocol);
            assert_eq!(levels, vec![expected; 512], "explicit commands must retain wire order");
            assert!(!terminated);
            if let Some(last) = last_sequence {
                assert_eq!(sequence, last + 1);
            }
            last_sequence = Some(sequence);
        }
        until(|| receipts.iter().all(|receipt| receipt.status() == SendStatus::Sent));
        // No further command is sent: only the worker's own stream deadline can emit this.
        let (sequence, levels, terminated) = data(&receiver, protocol);
        assert_eq!(sequence, last_sequence.unwrap() + 1);
        assert_eq!(levels, vec![0; 512]);
        assert!(!terminated);
        let stopped = Instant::now();
        transport.stop();
        assert!(
            stopped.elapsed() < Duration::from_millis(100),
            "the engine must not join termination"
        );
        if protocol == DmxProtocol::Sacn {
            let mut terminations = 0;
            while terminations < 3 {
                let (_, levels, terminated) = data(&receiver, protocol);
                assert_eq!(levels, vec![0; 512]);
                if terminated {
                    terminations += 1;
                }
                assert!(stopped.elapsed() < Duration::from_secs(1));
            }
        }
    }
}
