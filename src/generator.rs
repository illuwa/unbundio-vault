//! Password generator: cryptographically random, configurable character classes.

use rand::{rngs::OsRng, seq::SliceRandom};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GenError {
    #[error("no character class enabled")]
    NoClass,
    #[error("length {0} is smaller than the number of required classes ({1})")]
    TooShort(usize, usize),
    #[error("length must be 1..=256")]
    BadLength,
}

#[derive(Debug, Clone)]
pub struct GenOptions {
    pub length: usize,
    pub uppercase: bool,
    pub lowercase: bool,
    pub digits: bool,
    pub symbols: bool,
    pub exclude_ambiguous: bool,
}

impl Default for GenOptions {
    fn default() -> Self {
        Self {
            length: 20,
            uppercase: true,
            lowercase: true,
            digits: true,
            symbols: true,
            exclude_ambiguous: false,
        }
    }
}

const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.<>?/~";
/// Characters that confuse humans: 0/O, 1/l/I, etc.
const AMBIGUOUS: &[u8] = b"0O1lI|`'\"";

fn filtered(set: &[u8], exclude_ambiguous: bool) -> Vec<u8> {
    if !exclude_ambiguous {
        return set.to_vec();
    }
    set.iter()
        .copied()
        .filter(|c| !AMBIGUOUS.contains(c))
        .collect()
}

/// Generate one password. Guarantees at least one char from each enabled class.
pub fn generate(opts: &GenOptions) -> Result<String, GenError> {
    if opts.length == 0 || opts.length > 256 {
        return Err(GenError::BadLength);
    }
    let mut classes: Vec<Vec<u8>> = Vec::new();
    if opts.uppercase {
        classes.push(filtered(UPPER, opts.exclude_ambiguous));
    }
    if opts.lowercase {
        classes.push(filtered(LOWER, opts.exclude_ambiguous));
    }
    if opts.digits {
        classes.push(filtered(DIGITS, opts.exclude_ambiguous));
    }
    if opts.symbols {
        classes.push(filtered(SYMBOLS, opts.exclude_ambiguous));
    }
    classes.retain(|c| !c.is_empty());
    if classes.is_empty() {
        return Err(GenError::NoClass);
    }
    if opts.length < classes.len() {
        return Err(GenError::TooShort(opts.length, classes.len()));
    }

    let mut rng = OsRng;
    let mut out: Vec<u8> = Vec::with_capacity(opts.length);
    // One mandatory char per class.
    for class in &classes {
        let pick = class.choose(&mut rng).copied().ok_or(GenError::NoClass)?;
        out.push(pick);
    }
    // Fill the rest from the union.
    let union: Vec<u8> = classes.concat();
    while out.len() < opts.length {
        let pick = union.choose(&mut rng).copied().ok_or(GenError::NoClass)?;
        out.push(pick);
    }
    out.shuffle(&mut rng);
    Ok(String::from_utf8_lossy(&out).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn default_length_and_classes() {
        let opts = GenOptions::default();
        let pw = generate(&opts).expect("generate");
        assert_eq!(pw.len(), 20);
        assert!(pw.bytes().any(|b| UPPER.contains(&b)));
        assert!(pw.bytes().any(|b| LOWER.contains(&b)));
        assert!(pw.bytes().any(|b| DIGITS.contains(&b)));
        assert!(pw.bytes().any(|b| SYMBOLS.contains(&b)));
    }

    #[test]
    fn uniqueness() {
        let opts = GenOptions::default();
        let set: HashSet<String> = (0..50)
            .map(|_| generate(&opts).expect("generate"))
            .collect();
        assert!(set.len() > 45, "generator must be random");
    }

    #[test]
    fn digits_only() {
        let opts = GenOptions {
            length: 12,
            uppercase: false,
            lowercase: false,
            digits: true,
            symbols: false,
            exclude_ambiguous: false,
        };
        let pw = generate(&opts).expect("generate");
        assert!(pw.bytes().all(|b| DIGITS.contains(&b)));
    }

    #[test]
    fn rejects_empty_and_too_short() {
        let no_class = GenOptions {
            uppercase: false,
            lowercase: false,
            digits: false,
            symbols: false,
            ..Default::default()
        };
        assert!(generate(&no_class).is_err());
        let short = GenOptions {
            length: 2,
            ..Default::default()
        };
        assert!(generate(&short).is_err());
    }
}
