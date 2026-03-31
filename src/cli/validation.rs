const MAX_MODULE_PATH_LEN: usize = 512;

/// Validate that the module path contains only legal Rust identifier characters
/// and `::` separators.
///
/// # Errors
///
/// Returns an error if:
/// - The path exceeds 512 characters
/// - The path is empty
/// - Any character is not alphanumeric, `_`, or `:`
/// - The path contains `:::` or starts/ends with `::`
pub(super) fn validate_module_path(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Err(String::from("module path cannot be empty"));
    }
    if s.len() > MAX_MODULE_PATH_LEN {
        return Err(format!(
            "module path too long ({} chars, max {MAX_MODULE_PATH_LEN})",
            s.len()
        ));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':' || c == '-')
    {
        return Err(format!(
            "module path '{s}' contains invalid characters \
             (only a-z, A-Z, 0-9, _, - and :: are allowed)"
        ));
    }
    if s.starts_with("::") || s.ends_with("::") || s.contains(":::") {
        return Err(format!("module path '{s}' has malformed '::' separator"));
    }
    Ok(s.to_string())
}

/// Validate that the depth argument is a positive integer
///
/// # Errors
///
/// Returns an error if:
/// - The input is not a valid number
/// - The depth value is less than 1
pub(super) fn validate_depth(s: &str) -> Result<usize, String> {
    let value: usize = s
        .parse()
        .map_err(|_| format!("'{s}' is not a valid number"))?;
    if value < 1 {
        Err(String::from("depth must be at least 1"))
    } else {
        Ok(value)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_module_path_valid() {
        assert!(validate_module_path("foo").is_ok());
        assert!(validate_module_path("foo::bar").is_ok());
        assert!(validate_module_path("foo::bar::baz").is_ok());
        assert!(validate_module_path("my_mod::SomeType").is_ok());
        assert!(validate_module_path("my-crate::module").is_ok()); // hyphenated crate names
    }

    #[test]
    fn test_validate_module_path_rejects_empty() {
        assert!(validate_module_path("").is_err());
    }

    #[test]
    fn test_validate_module_path_rejects_too_long() {
        let long = "a".repeat(513);
        assert!(validate_module_path(&long).is_err());
    }

    #[test]
    fn test_validate_module_path_rejects_slash() {
        assert!(validate_module_path("foo/bar").is_err());
    }

    #[test]
    fn test_validate_module_path_rejects_dotdot() {
        assert!(validate_module_path("foo::..::bar").is_err());
    }

    #[test]
    fn test_validate_module_path_rejects_malformed_colons() {
        assert!(validate_module_path("::foo").is_err());
        assert!(validate_module_path("foo::").is_err());
        assert!(validate_module_path("foo:::bar").is_err());
    }

    #[test]
    fn test_validate_depth_valid() {
        assert_eq!(validate_depth("1").unwrap(), 1);
        assert_eq!(validate_depth("5").unwrap(), 5);
        assert_eq!(validate_depth("100").unwrap(), 100);
    }

    #[test]
    fn test_validate_depth_zero_rejected() {
        let result = validate_depth("0");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "depth must be at least 1");
    }

    #[test]
    fn test_validate_depth_invalid_number() {
        let result = validate_depth("abc");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a valid number"));
    }

    #[test]
    fn test_validate_depth_negative() {
        let result = validate_depth("-1");
        assert!(result.is_err());
    }
}
