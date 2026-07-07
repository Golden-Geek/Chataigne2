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
    let _guard = test_lock();
    clear();
    set_max_entries(DEFAULT_LOG_MAX_ENTRIES);

    let first = crate::log!(tag = "perf", level = info; "duplicate");
    let second = crate::log!(tag = "perf", level = info; "duplicate");

    assert_eq!(first.id, second.id);
    assert_eq!(second.repeat_count, 2);
    assert_eq!(records().len(), 1);

    let pending = drain_pending();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].repeat_count, 2);

    clear();
    set_max_entries(DEFAULT_LOG_MAX_ENTRIES);
}

#[test]
fn logger_max_entries_counts_collapsed_runs() {
    let _guard = test_lock();
    clear();
    set_max_entries(2);

    for _ in 0..64 {
        crate::log!(tag = "same", level = warning; "same");
    }
    crate::log!(tag = "next", level = warning; "next");
    crate::log!(tag = "last", level = warning; "last");

    let retained = records();
    assert_eq!(retained.len(), 2);
    assert_eq!(retained[0].tag, "next");
    assert_eq!(retained[1].tag, "last");

    clear();
    set_max_entries(DEFAULT_LOG_MAX_ENTRIES);
}

#[test]
fn process_output_prefix_includes_origin_when_present() {
    assert_eq!(
        process_output_prefix(LogLevel::Warning, "script", Some(NodeId(7))),
        "[golden][warning][script][node=7]"
    );
}
