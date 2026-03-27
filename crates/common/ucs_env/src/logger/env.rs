#[macro_export]
macro_rules! service_name {
    () => {
        env!("CARGO_BIN_NAME")
    };
}

#[macro_export]
macro_rules! git_describe {
    () => {
        env!("VERGEN_GIT_DESCRIBE")
    };
}

#[macro_export]
macro_rules! version {
    () => {
        concat!(
            env!("VERGEN_GIT_DESCRIBE"),
            "-",
            env!("VERGEN_GIT_SHA"),
            "-",
            env!("VERGEN_GIT_COMMIT_TIMESTAMP"),
        )
    };
}
