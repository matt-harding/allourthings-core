/// Convert a name to a URL/filesystem-safe slug.
///
/// Algorithm (canonical — matches STORAGE_SPEC.md §4):
///   1. Lowercase the entire string
///   2. Replace one or more consecutive non-[a-z0-9] characters with a single "-"
///   3. Strip leading and trailing "-"
///   4. Truncate to 50 characters
///   5. Strip any trailing "-" introduced by truncation
pub fn to_slug(name: &str) -> String {
    let lower = name.to_lowercase();

    // Replace runs of non-alphanumeric characters with a single hyphen
    let mut slug = String::with_capacity(lower.len());
    let mut last_was_hyphen = false;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            slug.push('-');
            last_was_hyphen = true;
        }
    }

    // Strip leading/trailing hyphens
    let slug = slug.trim_matches('-');

    // Truncate to 50 chars on a char boundary, then strip trailing hyphen
    let truncated = if slug.chars().count() > 50 {
        // Safe: all chars are ASCII at this point
        &slug[..50]
    } else {
        slug
    };

    truncated.trim_end_matches('-').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test vectors from STORAGE_SPEC.md §4 and MANIFEST.json
    #[test]
    fn slug_vectors() {
        let cases = [
            ("Bosch Washing Machine", "bosch-washing-machine"),
            ("PlayStation 5", "playstation-5"),
            ("100% Wool Blanket", "100-wool-blanket"),
            ("  Spaces  ", "spaces"),
            ("A & B -- C", "a-b-c"),
            ("MacBook Pro 14\"", "macbook-pro-14"),
            (
                "A very long name that exceeds fifty characters totally",
                "a-very-long-name-that-exceeds-fifty-characters-tot",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(to_slug(input), expected, "input: {:?}", input);
        }
    }

    #[test]
    fn empty_string() {
        assert_eq!(to_slug(""), "");
    }

    #[test]
    fn only_special_chars() {
        assert_eq!(to_slug("---"), "");
    }
}
