// Small helpers for turning raw request input into safe, normalised values:
// LIKE-pattern escaping, strict UUID parsing and PATCH-style optional text.

use serde::{Deserialize, Deserializer};
use uuid::Uuid;

use crate::error::{AppError, Result};

/// Escapes `\`, `%` and `_` so user input is matched literally inside a SQL
/// `LIKE`/`ILIKE` pattern. Every query that uses the result must say
/// `ESCAPE '\'` explicitly (Postgres' default escape is already `\`, but the
/// clause keeps the intent visible and independent of defaults).
pub fn escape_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for c in input.chars() {
        if matches!(c, '\\' | '%' | '_') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `%<escaped input>%` — a literal "contains" pattern for `ILIKE ... ESCAPE '\'`.
pub fn like_contains(input: &str) -> String {
    format!("%{}%", escape_like(input))
}

/// Parses a required UUID request field; anything else is a 400 naming the field.
pub fn parse_uuid(raw: &str, what: &str) -> Result<Uuid> {
    Uuid::parse_str(raw.trim()).map_err(|_| AppError::BadRequest(format!("Invalid {what}")))
}

/// Parses an optional UUID: absent/blank is `None`, malformed is a 400 (never
/// silently dropped, which would turn a typo into "no filter").
pub fn parse_optional_uuid(raw: Option<&str>, what: &str) -> Result<Option<Uuid>> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => parse_uuid(s, what).map(Some),
    }
}

/// Serde helper for PATCH-style optional text fields, to be used as
/// `#[serde(default, deserialize_with = "patch_text")] field: Option<Option<String>>`:
///
/// * field omitted      -> `None`          (keep the stored value)
/// * `null`, `""`, `"  "` -> `Some(None)`  (clear it)
/// * `"text"`           -> `Some(Some("text"))` (trimmed)
pub fn patch_text<'de, D>(d: D) -> std::result::Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = Option::<String>::deserialize(d)?;
    Ok(Some(
        v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
    ))
}

/// Applies a [`patch_text`] value on top of the stored one.
pub fn apply_patch(patch: Option<Option<String>>, existing: Option<String>) -> Option<String> {
    match patch {
        Some(v) => v,
        None => existing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct P {
        #[serde(default, deserialize_with = "patch_text")]
        d: Option<Option<String>>,
    }

    fn parse(json: &str) -> Option<Option<String>> {
        serde_json::from_str::<P>(json).unwrap().d
    }

    #[test]
    fn escapes_wildcards_and_backslash() {
        assert_eq!(escape_like("100%_a\\b"), "100\\%\\_a\\\\b");
        assert_eq!(like_contains("a_b"), "%a\\_b%");
        assert_eq!(escape_like("plain"), "plain");
    }

    #[test]
    fn uuid_parsing_is_strict() {
        assert!(parse_uuid("nope", "batch ID").is_err());
        assert!(parse_optional_uuid(Some("  "), "x").unwrap().is_none());
        assert!(parse_optional_uuid(None, "x").unwrap().is_none());
        assert!(parse_optional_uuid(Some("zzz"), "x").is_err());
        let id = Uuid::new_v4();
        assert_eq!(
            parse_optional_uuid(Some(&id.to_string()), "x").unwrap(),
            Some(id)
        );
    }

    #[test]
    fn patch_text_distinguishes_absent_null_and_empty() {
        assert_eq!(parse("{}"), None);
        assert_eq!(parse(r#"{"d":null}"#), Some(None));
        assert_eq!(parse(r#"{"d":""}"#), Some(None));
        assert_eq!(parse(r#"{"d":"  "}"#), Some(None));
        assert_eq!(parse(r#"{"d":" hi "}"#), Some(Some("hi".to_string())));
        assert_eq!(apply_patch(None, Some("k".into())), Some("k".into()));
        assert_eq!(apply_patch(Some(None), Some("k".into())), None);
    }
}
