// ---------------------------------------------------------------------------
// Placeholder tests — all term commands are now implemented (012-term-core)
// so this file exists only as a removal marker. No remaining commands use
// the placeholder path. project archive uses error-based not_implemented
// (exit 64 via stderr), not the placeholder envelope.
// ---------------------------------------------------------------------------

/// Placeholder path is no longer used by any command.
/// This test is preserved to document that fact.
#[test]
fn no_commands_use_placeholder_path() {
    // All previously-placeholder commands are now real commands:
    // - term * (5 subcommands) — implemented in 012-term-core
    // - source * (6 subcommands) — implemented in 011-source-core
    // - asset * (4 subcommands) — implemented in 010-asset-core
    // - project new/list/status/lint/index — implemented in 008 project lifecycle
    // - build, config, publish run/update — implemented in 006/009
    // - project archive returns an error-based not_implemented (stderr, not placeholder path)
}
