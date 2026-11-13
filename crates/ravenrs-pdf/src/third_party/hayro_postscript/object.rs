use crate::third_party::hayro_postscript::array::{self, Array};
use crate::third_party::hayro_postscript::error::{Error, Result};
use crate::third_party::hayro_postscript::name::{self, Name};
use crate::third_party::hayro_postscript::number::{self, Number};
use crate::third_party::hayro_postscript::reader::Reader;
use crate::third_party::hayro_postscript::string::{self, String};

/// A PostScript object.
#[derive(Debug, Clone, PartialEq)]
pub enum Object<'a> {
    /// A number object.
    Number(Number),
    /// A name object.
    Name(Name<'a>),
    /// A string object.
    String(String<'a>),
    /// An array object.
    Array(Array<'a>),
}

pub(crate) fn read<'a>(r: &mut Reader<'a>) -> Result<Object<'a>> {
    // PdfCraft patch: `<<` and `>>` are skipped by looping instead of
    // recursing once per delimiter, so a long run cannot overflow the stack.
    loop {
        skip_whitespace_and_comments(r);

        let b = r.peek_byte().ok_or(Error::SyntaxError)?;

        return match b {
            b'(' => string::parse_literal(r).map(|s| Object::String(String::from_literal(s))).ok_or(Error::SyntaxError),
            b'<' => {
                if r.peek_bytes(2) == Some(b"<~") {
                    string::parse_ascii85(r).map(|s| Object::String(String::from_ascii85(s))).ok_or(Error::SyntaxError)
                } else if r.peek_bytes(2) == Some(b"<<") {
                    // TODO: Proper dict support. For now, skip `<<` and return
                    // the next object so callers see the inner tokens.
                    r.forward();
                    r.forward();
                    continue;
                } else {
                    string::parse_hex(r).map(|s| Object::String(String::from_hex(s))).ok_or(Error::SyntaxError)
                }
            }
            b'/' => name::parse_literal(r).map(|s| Object::Name(Name::new(s, true))).ok_or(Error::SyntaxError),
            b'[' => array::parse(r).map(|d| Object::Array(Array::new(d))),
            b'>' => {
                if r.peek_bytes(2) == Some(b">>") {
                    // TODO: Proper dict support. Skip `>>` closing delimiter.
                    r.forward();
                    r.forward();
                    continue;
                } else {
                    Err(Error::SyntaxError)
                }
            }
            b'{' => {
                r.forward();
                Err(Error::UnsupportedType)
            }
            b'.' | b'+' | b'-' | b'0'..=b'9' => number::read(r).map(Object::Number),
            _ => name::parse_executable(r).map(|s| Object::Name(Name::new(s, false))).ok_or(Error::SyntaxError),
        };
    }
}

pub(crate) fn at_end(r: &mut Reader<'_>) -> bool {
    skip_whitespace_and_comments(r);
    r.peek_byte().is_none()
}

pub(crate) fn skip_whitespace_and_comments(r: &mut Reader<'_>) {
    loop {
        match r.peek_byte() {
            Some(b) if crate::third_party::hayro_postscript::reader::is_whitespace(b) => {
                r.forward();
            }
            Some(b'%') => {
                r.forward();
                r.forward_while(|b| !crate::third_party::hayro_postscript::reader::is_eol(b));
            }
            _ => return,
        }
    }
}

#[cfg(any())]
mod tests {
    use super::*;

    fn read_one(input: &[u8]) -> Result<Object<'_>> {
        let mut r = Reader::new(input);
        read(&mut r)
    }

    fn read_ok(input: &[u8]) -> Object<'_> {
        read_one(input).unwrap()
    }

    fn read_err(input: &[u8]) -> Error {
        read_one(input).unwrap_err()
    }

    #[cfg(any())]
    fn integer() {
        assert_eq!(read_ok(b"42 "), Object::Number(Number::Integer(42)));
    }

    #[cfg(any())]
    fn negative_integer() {
        assert_eq!(read_ok(b"-7 "), Object::Number(Number::Integer(-7)));
    }

    #[cfg(any())]
    fn literal_name() {
        assert_eq!(read_ok(b"/CMapName "), Object::Name(Name::new(b"CMapName", true)));
    }

    #[cfg(any())]
    fn executable_name() {
        let obj = read_ok(b"beginbfchar ");
        assert_eq!(obj, Object::Name(Name::new(b"beginbfchar", false)));
    }

    #[cfg(any())]
    fn literal_string() {
        assert_eq!(read_ok(b"(Hello)"), Object::String(String::from_literal(b"Hello")));
    }

    #[cfg(any())]
    fn hex_string() {
        assert_eq!(read_ok(b"<48656C6C6F>"), Object::String(String::from_hex(b"48656C6C6F")));
    }

    #[cfg(any())]
    fn ascii85_string() {
        assert_eq!(read_ok(b"<~87cURDZ~>"), Object::String(String::from_ascii85(b"87cURDZ")));
    }

    #[cfg(any())]
    fn array_simple() {
        let obj = read_ok(b"[1 2 3]");
        assert_eq!(obj, Object::Array(Array::new(b"1 2 3")));
    }

    #[cfg(any())]
    fn stray_close_bracket() {
        assert_eq!(read_err(b"]"), Error::SyntaxError);
    }

    #[cfg(any())]
    fn stray_gt() {
        assert_eq!(read_err(b">x"), Error::SyntaxError);
    }

    #[cfg(any())]
    fn skips_whitespace() {
        assert_eq!(read_ok(b"  \t\n 42 "), Object::Number(Number::Integer(42)));
    }

    #[cfg(any())]
    fn skips_comments() {
        assert_eq!(read_ok(b"% this is a comment\n42 "), Object::Number(Number::Integer(42)));
    }

    #[cfg(any())]
    fn eof() {
        assert!(read_one(b"").is_err());
        assert!(read_one(b"   ").is_err());
        assert!(read_one(b"% comment only\n").is_err());
    }

    // PdfCraft patch: `<<` and `>>` runs are skipped without recursion.
    #[cfg(any())]
    fn long_open_dict_run_does_not_overflow() {
        let input = alloc::vec![b'<'; 1_000_000];
        assert_eq!(read_err(&input), Error::SyntaxError);
    }

    #[cfg(any())]
    fn long_close_dict_run_does_not_overflow() {
        let input = b">>".repeat(500_000);
        assert_eq!(read_err(&input), Error::SyntaxError);
    }

    #[cfg(any())]
    fn dict_delimiters_are_skipped() {
        let mut r = Reader::new(b"<< /A 1 >>");
        assert_eq!(read(&mut r), Ok(Object::Name(Name::new(b"A", true))));
        assert_eq!(read(&mut r), Ok(Object::Number(Number::Integer(1))));
        assert_eq!(read(&mut r), Err(Error::SyntaxError));
    }

    #[cfg(any())]
    fn nested_dict_delimiters_are_skipped() {
        let mut r = Reader::new(b"<<<< /X >>>> 7 <<\n% c\n<< >> 8");
        assert_eq!(read(&mut r), Ok(Object::Name(Name::new(b"X", true))));
        assert_eq!(read(&mut r), Ok(Object::Number(Number::Integer(7))));
        assert_eq!(read(&mut r), Ok(Object::Number(Number::Integer(8))));
        assert!(at_end(&mut r));
    }
}
