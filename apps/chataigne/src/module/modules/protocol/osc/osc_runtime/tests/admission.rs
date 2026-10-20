use super::super::admission::{outbound_weight, packet_shape};
use super::*;
use rosc::OscBundle;

fn message(payload: OscValuePayload) -> OscOutboundMessage {
    OscOutboundMessage {
        address: Arc::from("/admission"),
        payload,
        remote_address: "127.0.0.1:45001".parse().unwrap(),
    }
}

#[test]
fn outgoing_admission_counts_string_and_argument_spare_storage() {
    assert!(outbound_weight(&message(OscValuePayload::Single(ParamValue::Int(1)))).is_ok());
    let value = ParamValue::Str(String::with_capacity(128 * 1024));
    assert!(outbound_weight(&message(OscValuePayload::Single(value))).is_err());
    let values = Vec::with_capacity(1025);
    assert!(outbound_weight(&message(OscValuePayload::Arguments(values))).is_err());
    let values = vec![ParamValue::Vec3(0.0, 0.0, 0.0); 400];
    assert!(outbound_weight(&message(OscValuePayload::Arguments(values))).is_err());
}

#[test]
fn packet_shape_accepts_normal_nested_bundles_and_arrays() {
    let packet = OscPacket::Bundle(OscBundle {
        timetag: (0, 1).into(),
        content: vec![OscPacket::Bundle(OscBundle {
            timetag: (0, 1).into(),
            content: vec![OscPacket::Message(OscMessage {
                addr: "/nested".into(),
                args: vec![OscType::Array(rosc::OscArray {
                    content: vec![OscType::Int(1)],
                })],
            })],
        })],
    });
    let bytes = encoder::encode(&packet).unwrap();
    assert!(packet_shape(&bytes).is_ok());
    assert!(decoder::decode_udp(&bytes).is_ok());
}

#[test]
fn packet_shape_rejects_recursive_bundles_before_the_decoder_allocates() {
    let mut bytes = encoder::encode(&OscPacket::Message(OscMessage {
        addr: "/depth".into(),
        args: vec![],
    }))
    .unwrap();
    for _ in 0..34 {
        let mut bundle = b"#bundle\0\0\0\0\0\0\0\0\x01".to_vec();
        bundle.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        bundle.extend_from_slice(&bytes);
        bytes = bundle;
    }
    assert!(packet_shape(&bytes).is_err());
}

#[test]
fn packet_shape_bounds_array_depth_bundle_width_and_truncated_elements() {
    let mut bytes = b"/array\0\0".to_vec();
    bytes.push(b',');
    bytes.extend(std::iter::repeat_n(b'[', 33));
    bytes.extend(std::iter::repeat_n(b']', 33));
    bytes.push(0);
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
    assert!(packet_shape(&bytes).is_err());
    let packet = OscPacket::Message(OscMessage {
        addr: "/width".into(),
        args: vec![],
    });
    let bundle = OscPacket::Bundle(OscBundle {
        timetag: (0, 1).into(),
        content: vec![packet; 257],
    });
    assert!(packet_shape(&encoder::encode(&bundle).unwrap()).is_err());
    assert!(packet_shape(b"#bundle\0\0\0\0\0\0\0\0\x01\0\0\0\x08abc").is_err());
}
