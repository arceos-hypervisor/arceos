#[derive(Debug, PartialEq)]
pub enum UtilError {
    NumOverflow { start: usize, size: usize },
    NumParseIntError {},
    NumInvalid { num: usize },
}

impl core::fmt::Display for UtilError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            UtilError::NumOverflow { start, size } => {
                write!(f, "range overflows: start {}, size {}", start, size)
            }
            UtilError::NumParseIntError {} => write!(f, "failed to parse integer"),
            UtilError::NumInvalid { num } => write!(f, "invalid number: {}", num),
        }
    }
}

pub type UtilResult<T, E = UtilError> = core::result::Result<T, E>;
