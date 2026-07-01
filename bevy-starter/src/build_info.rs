pub const COMMIT: &str = match option_env!("BEVY_STARTER_BUILD_COMMIT") {
    Some(c) => c,
    None => "unknown",
};

pub const BUILT_AT: &str = match option_env!("BEVY_STARTER_BUILD_TIME") {
    Some(t) => t,
    None => "unknown",
};
