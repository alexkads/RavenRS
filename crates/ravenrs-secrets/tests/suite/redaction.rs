use ravenrs_secrets::{MARK, Redactor, redact};

use crate::{TOKEN, vault};

#[test]
fn a_value_used_never_comes_out_in_text() {
    let vault = vault(1);
    let token = vault.put("github-token", TOKEN, "ana").unwrap();
    let redactor = vault.redactor();
    let line =
        vault.use_with(&token, "ana", |value| format!("git push https://x:{value}@github.com")).unwrap();
    let written = redactor.redact(&line);
    assert_eq!(written, format!("git push https://x:{MARK}@github.com"));
    assert!(!format!("{redactor:?}").contains(TOKEN));
}

#[test]
fn the_longest_goes_first_and_short_ones_stay() {
    let values = vec!["abcd".to_owned(), "abcdefgh".to_owned(), "abc".to_owned()];
    assert_eq!(redact("abcdefgh abcd abc", &values), format!("{MARK} {MARK} abc"));
    let redactor = Redactor::new();
    redactor.add("ab");
    assert!(redactor.is_empty(), "too short to be a secret");
}
