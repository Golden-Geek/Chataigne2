use crate::app::module::common::mqtt::MqttQos;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttTransportConfig {
    pub remote_host: String,
    pub remote_port: u16,
    pub client_id: String,
    pub credentials: Option<MqttCredentials>,
    pub clean_session: bool,
    pub keep_alive_secs: u64,
    pub subscriptions: Vec<MqttSubscriptionConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttSubscriptionConfig {
    pub topic_filter: String,
    pub qos: MqttQos,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MqttReceivedPublish {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: MqttQos,
    pub retain: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum MqttConnectionStatus {
    Connected,
    Recovering { message: String },
}

#[derive(Clone, Debug)]
pub(crate) enum MqttWorkerEvent {
    Publish(golden_io::RetainedPayload<MqttReceivedPublish>),
    Status(MqttConnectionStatus),
    Error(String),
}
