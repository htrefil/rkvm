use crate::convert::Convert;
use crate::linux::glue;

use crate::sync::SyncEvent;

impl Convert for SyncEvent {
    type Raw = u16;

    fn to_raw(&self) -> Option<Self::Raw> {
        let raw = match self {
            Self::All => glue::SYN_REPORT,
            Self::Mt => glue::SYN_MT_REPORT,
        };

        Some(raw as _)
    }

    fn from_raw(raw: Self::Raw) -> Option<Self> {
        let event = match raw as _ {
            glue::SYN_REPORT => SyncEvent::All,
            glue::SYN_MT_REPORT => SyncEvent::Mt,
            _ => return None,
        };

        Some(event)
    }
}
