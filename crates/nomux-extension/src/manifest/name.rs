//! The name syntax of identifiers, slots and scopes.

/// A name starts with a lower-case letter and has only lower-case letters,
/// digits and `-`.
pub(crate) fn is_name(text: &str) -> bool {
    let mut bytes = text.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

/// One or more names with `.` between them.
pub(crate) fn is_dotted_name(text: &str) -> bool {
    text.split('.').all(is_name)
}
