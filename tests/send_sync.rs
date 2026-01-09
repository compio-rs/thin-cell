#![allow(dead_code)]

use thin_cell::ThinCell;

trait IsSend: Send {
    const IS_SEND: bool = true;
}

trait IsSync: Sync {
    const IS_SYNC: bool = true;
}

trait Fallback {
    const IS_SEND: bool = false;
    const IS_SYNC: bool = false;
}

impl<T: ?Sized> Fallback for T {}
impl<T: ?Sized + Send> IsSend for T {}
impl<T: ?Sized + Sync> IsSync for T {}

const fn test_send_sync() {
    if ThinCell::<()>::IS_SEND {
        panic!("ThinCell should not be send")
    }

    if ThinCell::<()>::IS_SYNC {
        panic!("ThinCell should not be send")
    }
}

const _: () = test_send_sync();

#[test]
fn test_thin_cell_not_send_sync() {
    test_send_sync()
}
