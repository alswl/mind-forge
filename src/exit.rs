#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ExitCode {
    Ok = 0,
    Failure = 1,
    UsageError = 2,
    NotImplemented = 64,
}

impl ExitCode {
    pub fn from(code: u8) -> Self {
        match code {
            0 => Self::Ok,
            1 => Self::Failure,
            2 => Self::UsageError,
            64 => Self::NotImplemented,
            _ => Self::Failure,
        }
    }
}
