use super::*;

#[test]
fn wsl_env_allowlist_without_initial_working_dir() {
    let wslenv = wsl_env_allowlist(false).to_string_lossy().into_owned();

    assert_eq!(
        wslenv.split(':').collect::<Vec<_>>(),
        vec![
            format!("{HONOR_PS1_NAME}/u"),
            format!("{USE_SSH_WRAPPER_NAME}/u"),
            format!("{SHELL_DEBUG_MODE_NAME}/u"),
            format!("{TERM_PROGRAM_NAME}/u"),
            format!("{IS_LOCAL_SESSION_NAME}/u"),
            format!("{SSH_SOCKET_DIR}/u"),
            format!("{CLIENT_VERSION_NAME}/u"),
            format!("{TERMINAL_SESSION_UUID_ENV}/u"),
            format!("{FOCUS_URL_ENV}/u"),
        ],
    );
}

#[test]
fn wsl_env_allowlist_includes_initial_working_dir() {
    let wslenv = wsl_env_allowlist(true).to_string_lossy().into_owned();

    assert_eq!(
        wslenv.split(':').collect::<Vec<_>>(),
        vec![
            format!("{HONOR_PS1_NAME}/u"),
            format!("{USE_SSH_WRAPPER_NAME}/u"),
            format!("{SHELL_DEBUG_MODE_NAME}/u"),
            format!("{TERM_PROGRAM_NAME}/u"),
            format!("{IS_LOCAL_SESSION_NAME}/u"),
            format!("{SSH_SOCKET_DIR}/u"),
            format!("{CLIENT_VERSION_NAME}/u"),
            format!("{TERMINAL_SESSION_UUID_ENV}/u"),
            format!("{FOCUS_URL_ENV}/u"),
            format!("{INITIAL_WORKING_DIR_NAME}/pu"),
        ],
    );
}
