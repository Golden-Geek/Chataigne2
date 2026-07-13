use super::*;

#[test]
fn logger_joins_multiple_parts() {
    let _guard = test_lock();
    clear();
    let record = crate::log!("hello", 42, "world");
    assert_eq!(record.level, LogLevel::Info);
    assert_eq!(record.tag, "general");
    assert_eq!(record.message, "hello\n42\nworld");
}

#[test]
fn logger_accepts_options() {
    let _guard = test_lock();
    clear();
    let record = crate::log!(tag = "ui", level = warning; "payload");
    assert_eq!(record.level, LogLevel::Warning);
    assert_eq!(record.tag, "ui");
}

#[test]
fn logger_accepts_success_options() {
    let _guard = test_lock();
    clear();
    let record = crate::log!(tag = "ui", level = success; "payload");
    assert_eq!(record.level, LogLevel::Success);
    assert_eq!(record.tag, "ui");
}

#[test]
fn logger_accepts_warning_shortcut() {
    let _guard = test_lock();
    clear();
    let record = crate::logwarning!("payload");
    assert_eq!(record.level, LogLevel::Warning);
    assert_eq!(record.tag, "general");
}

#[test]
fn logger_accepts_error_shortcut_options() {
    let _guard = test_lock();
    clear();
    let origin = NodeId(9);
    let record = crate::logerror!(origin = origin, tag = "ui"; "payload");
    assert_eq!(record.level, LogLevel::Error);
    assert_eq!(record.tag, "ui");
    assert_eq!(record.origin, Some(origin));
}

#[test]
fn logger_accepts_success_shortcut_options() {
    let _guard = test_lock();
    clear();
    let record = crate::logsuccess!(tag = "ui"; "payload");
    assert_eq!(record.level, LogLevel::Success);
    assert_eq!(record.tag, "ui");
}

#[test]
fn logger_uses_thread_local_origin_when_not_explicit() {
    let _guard = test_lock();
    clear();
    let origin = NodeId(12);

    let record = with_node_origin(origin, || crate::log!("from node"));
    assert_eq!(record.origin, Some(origin));
}

#[test]
fn logger_collapses_consecutive_duplicates_into_one_record() {
    let mut state = LoggerState::with_defaults();

    let first = state.push_message(1, LogLevel::Info, "perf".to_string(), None, "duplicate".to_string());
    let second = state.push_message(2, LogLevel::Info, "perf".to_string(), None, "duplicate".to_string());

    assert_eq!(first.id, second.id);
    assert_eq!(second.repeat_count, 2);
    assert_eq!(state.retained.len(), 1);

    let pending = state.pending.drain(..).collect::<Vec<_>>();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].repeat_count, 2);
}

#[test]
fn logger_max_entries_counts_collapsed_runs() {
    let mut state = LoggerState::with_defaults();
    state.max_entries = 2;

    for timestamp_ms in 0..64 {
        state.push_message(
            timestamp_ms,
            LogLevel::Warning,
            "same".to_string(),
            None,
            "same".to_string(),
        );
    }
    state.push_message(64, LogLevel::Warning, "next".to_string(), None, "next".to_string());
    state.push_message(65, LogLevel::Warning, "last".to_string(), None, "last".to_string());

    let retained = state.retained.into_iter().collect::<Vec<_>>();
    assert_eq!(retained.len(), 2);
    assert_eq!(retained[0].tag, "next");
    assert_eq!(retained[1].tag, "last");
}

#[test]
fn process_output_prefix_includes_origin_when_present() {
    assert_eq!(
        process_output_prefix(LogLevel::Warning, "script", Some(NodeId(7))),
        "[golden][warning][script][node=7]"
    );
}
