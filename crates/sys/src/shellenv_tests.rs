//! Unit tests for parsing `env -0` output — pure string handling, no shell.

use super::{is_var_name, parse_env0};

/// Joins `entries` the way `env -0` writes them: each `KEY=VALUE` followed by a NUL.
fn env0(entries: &[&str]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries {
        bytes.extend_from_slice(entry.as_bytes());
        bytes.push(0);
    }
    bytes
}

#[test]
fn parses_nul_delimited_entries() {
    let parsed = parse_env0(&env0(&["PATH=/usr/bin:/bin", "HOME=/home/dev"]));
    assert_eq!(parsed.get("PATH"), Some(&"/usr/bin:/bin".to_string()));
    assert_eq!(parsed.get("HOME"), Some(&"/home/dev".to_string()));
}

#[test]
fn keeps_values_containing_equals_and_newlines() {
    // NUL delimiting is what makes this unambiguous: the value runs to the next NUL, so an
    // embedded `=` or newline is preserved rather than splitting the entry.
    let parsed = parse_env0(&env0(&["EXPR=a=b=c", "MULTI=line1\nline2"]));
    assert_eq!(parsed.get("EXPR"), Some(&"a=b=c".to_string()));
    assert_eq!(parsed.get("MULTI"), Some(&"line1\nline2".to_string()));
}

#[test]
fn drops_session_bookkeeping_variables() {
    // The capturing shell's own PWD/OLDPWD/SHLVL/_ must not leak into a child.
    let parsed = parse_env0(&env0(&[
        "PWD=/somewhere",
        "OLDPWD=/before",
        "SHLVL=3",
        "_=/usr/bin/env",
        "KEEP=yes",
    ]));
    assert_eq!(parsed.get("KEEP"), Some(&"yes".to_string()));
    for dropped in ["PWD", "OLDPWD", "SHLVL", "_"] {
        assert!(!parsed.contains_key(dropped), "{dropped} should be dropped");
    }
}

#[test]
fn discards_non_variable_noise() {
    // An rc file that prints a banner to stdout produces a chunk with no valid name; it is
    // discarded rather than added as a junk variable.
    let parsed = parse_env0(&env0(&["welcome to the shell!", "REAL=value"]));
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed.get("REAL"), Some(&"value".to_string()));
}

#[test]
fn validates_variable_names() {
    assert!(is_var_name("PATH"));
    assert!(is_var_name("_UNDERSCORE"));
    assert!(is_var_name("MIXED_123"));
    assert!(!is_var_name("1LEADING_DIGIT"));
    assert!(!is_var_name("has space"));
    assert!(!is_var_name("has-dash"));
    assert!(!is_var_name(""));
}
