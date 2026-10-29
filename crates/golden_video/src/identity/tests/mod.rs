use super::*;

#[test]
fn offline_identity_serde_is_transparent_and_rejects_zero() {
    for value in [1, u64::MAX] {
        let json = value.to_string();
        assert_eq!(serde_json::to_string(&TextureId::new(value).unwrap()).unwrap(), json);
        assert_eq!(serde_json::from_str::<TextureId>(&json).unwrap().get(), value);
        assert_eq!(serde_json::from_str::<OutputId>(&json).unwrap().get(), value);
        assert_eq!(serde_json::from_str::<PassId>(&json).unwrap().get(), value);
    }
    assert!(serde_json::from_str::<TextureId>("0").is_err());
    assert!(serde_json::from_str::<OutputId>("0").is_err());
    assert!(serde_json::from_str::<PassId>("0").is_err());
    assert!(serde_json::from_str::<TextureId>("-1").is_err());
}
