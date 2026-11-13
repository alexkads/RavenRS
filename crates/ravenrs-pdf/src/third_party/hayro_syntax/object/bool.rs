//! Booleans.

use crate::third_party::hayro_syntax::object::Object;
use crate::third_party::hayro_syntax::object::macros::object;
use crate::third_party::hayro_syntax::reader::Reader;
use crate::third_party::hayro_syntax::reader::{Readable, ReaderContext, ReaderExt, Skippable};

impl Skippable for bool {
    fn skip(r: &mut Reader<'_>, _: bool) -> Option<()> {
        match r.peek_byte()? {
            b't' => r.forward_tag(b"true"),
            b'f' => r.forward_tag(b"false"),
            _ => None,
        }
    }
}

impl Readable<'_> for bool {
    fn read(r: &mut Reader<'_>, _: &ReaderContext<'_>) -> Option<Self> {
        match r.skip::<Self>(true)? {
            b"true" => Some(true),
            b"false" => Some(false),
            _ => None,
        }
    }
}

object!(bool, Boolean);

#[cfg(any())]
mod tests {
    use crate::third_party::hayro_syntax::reader::Reader;
    use crate::third_party::hayro_syntax::reader::ReaderExt;

    #[cfg(any())]
    fn bool_true() {
        assert!(Reader::new("true".as_bytes()).read_without_context::<bool>().unwrap());
    }

    #[cfg(any())]
    fn bool_false() {
        assert!(!Reader::new("false".as_bytes()).read_without_context::<bool>().unwrap());
    }

    #[cfg(any())]
    fn bool_trailing() {
        assert!(Reader::new("trueabdf".as_bytes()).read_without_context::<bool>().unwrap());
    }
}
