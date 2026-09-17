use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};

pub fn hash_password(password: &str) -> Result<String, String> {
    // Argon2 0.6 generates a fresh OS-random salt through its default getrandom feature.
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|error| format!("Could not hash the password: {error}"))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    parsed.algorithm.as_str() == "argon2id"
        && Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
}

/// The separate hash key distinguishes hashes from all legacy plaintext values.
/// An existing hash is authoritative, even when malformed: never fall back to
/// the old plaintext password after a hash has been saved.
pub fn migrate_settings(
    settings: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<bool, String> {
    if !settings.contains_key("password") {
        return Ok(false);
    }
    if !settings.contains_key("password_hash") {
        let password = settings
            .get("password")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let hash = if password.is_empty() {
            String::new()
        } else {
            hash_password(password)?
        };
        settings.insert("password_hash".into(), serde_json::Value::String(hash));
    }
    settings.remove("password");
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_saved_with_argon2_0_5_still_verify() {
        // Generated with argon2 0.5.3 defaults and the test salt "locoryn-test-salt".
        let hash = "$argon2id$v=19$m=19456,t=2,p=1$bG9jb3J5bi10ZXN0LXNhbHQ$8QTHN1P+y/6DBUZhszz0cTPGAzkr/iHOcF4KMwOa4IE";
        assert!(verify_password("legacy password", hash));
        assert!(!verify_password("wrong password", hash));
    }

    #[test]
    fn passwords_use_unique_salts_and_verify_without_accepting_the_hash_itself() {
        let password = "  classroom 🔑  ";
        let first = hash_password(password).unwrap();
        let second = hash_password(password).unwrap();
        assert!(first.starts_with("$argon2id$v=19$"));
        assert_ne!(first, second);
        assert!(verify_password(password, &first));
        assert!(verify_password(password, &second));
        assert!(!verify_password("wrong", &first));
        assert!(!verify_password(password.trim(), &first));
        assert!(!verify_password(&first, &first));
        assert!(!verify_password("", ""));
        assert!(!verify_password("password", "$argon2id$invalid"));
    }

    #[test]
    fn migration_removes_plaintext_and_is_idempotent() {
        for enabled in [false, true] {
            let mut settings = serde_json::json!({
                "password": "legacy password", "password_enabled": enabled,
                "password_scope": "advanced_settings_only", "top_k": 17
            })
            .as_object()
            .unwrap()
            .clone();
            assert!(migrate_settings(&mut settings).unwrap());
            assert!(!settings.contains_key("password"));
            assert!(verify_password(
                "legacy password",
                settings["password_hash"].as_str().unwrap()
            ));
            assert_eq!(settings["password_enabled"], enabled);
            assert_eq!(settings["password_scope"], "advanced_settings_only");
            assert_eq!(settings["top_k"], 17);
            let migrated = settings.clone();
            assert!(!migrate_settings(&mut settings).unwrap());
            assert_eq!(settings, migrated);
        }
    }

    #[test]
    fn migration_never_replaces_an_existing_hash_with_legacy_plaintext() {
        let mut settings = serde_json::json!({
            "password_hash": "$argon2id$invalid", "password": "legacy"
        })
        .as_object()
        .unwrap()
        .clone();
        migrate_settings(&mut settings).unwrap();
        assert_eq!(settings["password_hash"], "$argon2id$invalid");
        assert!(!settings.contains_key("password"));
        assert!(!verify_password(
            "legacy",
            settings["password_hash"].as_str().unwrap()
        ));
    }
}
