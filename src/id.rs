use rand::RngCore;

/// Generate a new item ID: 4 cryptographically random bytes, hex-encoded lowercase (8 chars).
pub fn generate_id() -> String {
    let mut bytes = [0u8; 4];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_is_8_chars() {
        let id = generate_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn id_is_lowercase_hex() {
        let id = generate_id();
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }

    #[test]
    fn ids_are_unique() {
        let ids: Vec<_> = (0..100).map(|_| generate_id()).collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len());
    }
}
