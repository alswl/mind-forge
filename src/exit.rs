#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    Ok = 0,
    Failure = 1,
    UsageError = 2,
    NotImplemented = 64,
}
