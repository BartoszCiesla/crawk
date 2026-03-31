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
