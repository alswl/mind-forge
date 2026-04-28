use std::path::Path;

use assert_cmd::Command;

pub use tempfile::TempDir;

/// 构建一个在当前目录执行的 mf 命令
pub fn mf() -> Command {
    Command::cargo_bin("mf").expect("mf binary")
}

/// 在指定目录执行 mf 命令
pub fn mf_in(dir: impl AsRef<Path>) -> Command {
    let mut cmd = mf();
    cmd.current_dir(dir.as_ref());
    cmd
}

/// 在指定目录执行命令并返回 (stdout, stderr, exit_code)
pub fn run_in(dir: impl AsRef<Path>, args: &[&str]) -> (String, String, i32) {
    let output = mf_in(dir).args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}
