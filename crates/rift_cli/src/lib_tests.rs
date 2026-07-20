use clap::Parser;

use super::*;

#[test]
fn subcommand_without_global_flags_parses() {
    let args = Args::try_parse_from(["rift", "completions", "zsh"]).unwrap();

    assert!(!args.debug());
    assert!(matches!(
        args.command(),
        Some(Command::Completions {
            shell: Some(clap_complete::aot::Shell::Zsh)
        })
    ));
}

#[test]
fn debug_before_subcommand_parses() {
    // Regression test: `rift --debug <subcommand>` should work.
    // Global flags like --debug must not prevent subcommand detection —
    // previously the top-level [URLS] positional would swallow the subcommand
    // name, failing with `invalid value 'completions' for '[URLS]...'`.
    let args = Args::try_parse_from(["rift", "--debug", "completions", "zsh"]).unwrap();

    assert!(args.debug());
    assert!(matches!(
        args.command(),
        Some(Command::Completions {
            shell: Some(clap_complete::aot::Shell::Zsh)
        })
    ));
}

#[test]
fn debug_before_long_flag_subcommand_parses() {
    // `--dump-debug-info` is a `long_flag` subcommand, so `rift --debug
    // --dump-debug-info` must resolve the flag to the subcommand rather than
    // treating it as a stray top-level argument.
    let args = Args::try_parse_from(["rift", "--debug", "--dump-debug-info"]).unwrap();

    assert!(args.debug());
    assert!(matches!(args.command(), Some(Command::DumpDebugInfo)));
}

#[test]
fn debug_after_subcommand_still_parses() {
    // `--debug` is `global = true`, so it is also accepted after the
    // subcommand. Dropping `args_conflicts_with_subcommands` must not regress
    // this ordering.
    let args = Args::try_parse_from(["rift", "completions", "zsh", "--debug"]).unwrap();

    assert!(args.debug());
    assert!(matches!(
        args.command(),
        Some(Command::Completions {
            shell: Some(clap_complete::aot::Shell::Zsh)
        })
    ));
}

#[test]
fn deep_link_urls_still_parse() {
    // Deep links are plain positionals with no subcommand. This is the
    // behavior most at risk from `subcommand_precedence_over_arg`, so pin it.
    let args = Args::try_parse_from(["rift", "rift://action/new_tab?path=/tmp/foo"]).unwrap();

    assert!(args.command().is_none());
    let urls = &args.app_args().urls;
    assert_eq!(urls.len(), 1);
    assert_eq!(urls[0].scheme(), "rift");
    assert_eq!(urls[0].host_str(), Some("action"));
    assert_eq!(urls[0].path(), "/new_tab");
    assert_eq!(urls[0].query(), Some("path=/tmp/foo"));
}

#[test]
fn debug_before_deep_link_url_parses() {
    // A global flag ahead of a deep link must still leave the URL as a
    // positional rather than a subcommand.
    let args =
        Args::try_parse_from(["rift", "--debug", "rift://action/new_tab?path=/tmp/foo"]).unwrap();

    assert!(args.debug());
    assert!(args.command().is_none());
    assert_eq!(args.app_args().urls.len(), 1);
}

#[test]
fn multiple_deep_link_urls_parse() {
    let args = Args::try_parse_from([
        "rift",
        "rift://action/new_tab",
        "rift://action/new_tab?path=/tmp/foo",
    ])
    .unwrap();

    assert!(args.command().is_none());
    assert_eq!(args.app_args().urls.len(), 2);
}

#[test]
fn finish_update_flag_before_subcommand_parses() {
    // `--finish-update` lives on the flattened `AppArgs`, which is exactly the
    // combination `args_conflicts_with_subcommands` used to reject.
    let args = Args::try_parse_from(["rift", "--finish-update", "completions", "zsh"]).unwrap();

    assert!(args.app_args().finish_update);
    assert!(matches!(
        args.command(),
        Some(Command::Completions {
            shell: Some(clap_complete::aot::Shell::Zsh)
        })
    ));
}

#[test]
fn parent_pid_before_subcommand_parses() {
    let args =
        Args::try_parse_from(["rift", "--parent-pid", "4242", "completions", "zsh"]).unwrap();

    assert_eq!(args.app_args().parent.pid, Some(4242));
    assert!(matches!(
        args.command(),
        Some(Command::Completions {
            shell: Some(clap_complete::aot::Shell::Zsh)
        })
    ));
}

#[test]
fn no_args_parses_to_bare_app_launch() {
    let args = Args::try_parse_from(["rift"]).unwrap();

    assert!(!args.debug());
    assert!(args.command().is_none());
    assert!(args.app_args().urls.is_empty());
}
