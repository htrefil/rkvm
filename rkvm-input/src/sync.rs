use crate::glue;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum SyncEvent {
    All,
    Mt,
}

impl SyncEvent {
    pub(crate) fn to_raw(&self) -> u16 {
        let code = match self {
            Self::All => glue::SYN_REPORT,
            Self::Mt => glue::SYN_MT_REPORT,
        };

        code as _
    }
}
