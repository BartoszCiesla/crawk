/// Render orphan modules as a plain sorted list, one per line.
pub(crate) fn render_orphans(orphans: &[String]) -> String {
    if orphans.is_empty() {
        return String::new();
    }
    let mut out = String::new();
    for orphan in orphans {
        out.push_str(orphan);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_empty() {
        assert_eq!(render_orphans(&[]), "");
    }

    #[test]
    fn render_single() {
        assert_eq!(render_orphans(&["standalone".to_owned()]), "standalone\n");
    }

    #[test]
    fn render_multiple() {
        let orphans = vec!["alpha".to_owned(), "zebra".to_owned()];
        assert_eq!(render_orphans(&orphans), "alpha\nzebra\n");
    }
}
