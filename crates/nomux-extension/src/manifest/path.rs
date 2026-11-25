//! The path syntax of the manifest.

/// Returns `true` if the path has one or more parts with `/` between them,
/// and no part is empty, `.` or `..`.
pub(crate) fn has_plain_parts(path: &str) -> bool {
    path.split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Returns `true` if the path names a file in the extension package: it is
/// relative and cannot go out of the package.
pub(crate) fn is_package_path(path: &str) -> bool {
    !path.contains('\\') && has_plain_parts(path)
}

pub(crate) const PACKAGE_PATH_RULE: &str = "must be a relative path in the package, with `/` between the parts and without `.` or `..` parts";
