use super::*;

pub(super) fn publish_bytes(request: &MqttPublishRequest) -> Result<usize, String> {
    if request.topic.trim().is_empty() || !rumqttc::valid_topic(&request.topic) {
        return Err("Invalid MQTT publish topic".into());
    }
    if request.topic.capacity() > 4096
        || request.description.capacity() > 256
        || request.payload.capacity() > MAX_PACKET
        || request
            .topic
            .len()
            .saturating_add(request.payload.len())
            .saturating_add(16)
            > MAX_PACKET
    {
        return Err("MQTT publish exceeds 4096 topic bytes / 256 description bytes / 1 MiB packet".into());
    }
    // Original retained request plus rumqttc's owned copy, through acknowledgement
    // or native teardown. Native packet buffers have their separate packet/inflight caps.
    Ok(2 * (size_of::<MqttPublishRequest>()
        + request.topic.capacity()
        + request.payload.capacity()
        + request.description.capacity())
        + 1024)
}

pub(super) fn incoming_bytes(topic_capacity: usize, payload_capacity: usize) -> Option<usize> {
    (topic_capacity <= 4096 && payload_capacity <= MAX_PACKET)
        .then(|| size_of::<MqttReceivedPublish>() + 2 * size_of::<usize>() + topic_capacity + payload_capacity)
}

pub(super) fn config_bytes(config: &MqttTransportConfig) -> Result<usize, String> {
    let text = |value: &String, max: usize| value.capacity() <= max && !value.contains('\0');
    if !text(&config.remote_host, 1024)
        || config.remote_host.trim().is_empty()
        || !text(&config.client_id, 256)
        || config.client_id.is_empty()
        || !(1..=65535).contains(&config.keep_alive_secs)
        || config.subscriptions.len() > 128
        || config.subscriptions.capacity() > 128
    {
        return Err("MQTT configuration exceeds host/client/subscription/keepalive limits".into());
    }
    let mut bytes = size_of::<MqttTransportConfig>()
        + 2 * size_of::<usize>()
        + config.remote_host.capacity()
        + config.client_id.capacity()
        + config.subscriptions.capacity() * size_of::<MqttSubscriptionConfig>();
    if let Some(credentials) = &config.credentials {
        if !text(&credentials.username, 4096) || !text(&credentials.password, 4096) {
            return Err("MQTT credentials exceed 4096 bytes".into());
        }
        bytes += credentials.username.capacity() + credentials.password.capacity();
    }
    for subscription in &config.subscriptions {
        if !text(&subscription.topic_filter, 1024) || !rumqttc::valid_filter(&subscription.topic_filter) {
            return Err("MQTT subscription exceeds 1024 bytes or has an invalid filter".into());
        }
        bytes += subscription.topic_filter.capacity();
    }
    if bytes > 512 * 1024 {
        return Err("MQTT configuration exceeds 512 KiB".into());
    }
    // Retained source, worker configuration and library options/requests.
    Ok(3 * bytes + 1024)
}

pub(super) fn diagnostic(text: String) -> String {
    let mut end = text.len().min(1024);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
