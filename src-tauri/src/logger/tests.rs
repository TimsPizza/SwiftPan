use super::*;

#[test]
fn log_level_parsing_is_case_insensitive() {
    let cases = [
        ("trace", LogLevel::Trace),
        ("DEBUG", LogLevel::Debug),
        ("Info", LogLevel::Info),
        ("wArN", LogLevel::Warn),
        ("ERROR", LogLevel::Error),
    ];

    for (input, expected) in cases {
        assert_eq!(LogLevel::from_str(input), expected);
    }
}

#[test]
fn unknown_log_level_falls_back_to_info() {
    for input in ["", "verbose", "warning", "  debug  "] {
        assert_eq!(LogLevel::from_str(input), LogLevel::Info);
    }
}

#[test]
fn log_levels_have_monotonic_severity_order() {
    let levels = [
        LogLevel::Trace,
        LogLevel::Debug,
        LogLevel::Info,
        LogLevel::Warn,
        LogLevel::Error,
    ];

    for pair in levels.windows(2) {
        assert!(pair[0] < pair[1], "log severity order is not monotonic");
    }
}

#[test]
fn log_level_labels_are_stable_uppercase_contracts() {
    let cases = [
        (LogLevel::Trace, "TRACE"),
        (LogLevel::Debug, "DEBUG"),
        (LogLevel::Info, "INFO"),
        (LogLevel::Warn, "WARN"),
        (LogLevel::Error, "ERROR"),
    ];

    for (level, expected) in cases {
        assert_eq!(level.as_str(), expected);
    }
}
