//! The FR-014 update-check wake, read from `budgets.toml`.
//!
//! SC-005 enumerates every scheduled wake on the idle path in the budget file,
//! with its period and its processor-time bound, and T060 arms the update
//! check under that entry and no other. This module reads the entry; the
//! crate's build script runs it over the file and compiles the two numbers
//! in, so the period and the bound are the file's, and a file without the
//! entry fails the build. The unit tests below prove the reader the script
//! runs.

/// The `[[wake]]` entry's name the update check is armed under.
pub const UPDATE_CHECK: &str = "update check";

/// The two numbers the update check takes from its wake entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wake {
    /// Seconds between wakes.
    pub period_seconds: u64,
    /// Milliseconds of processor time one wake may use.
    pub processor_time_bound_ms: u64,
}

/// Reads the `[[wake]]` entry named `name` from the text of `budgets.toml`.
///
/// The file is TOML, and this reads only what a wake entry is: a
/// `[[wake]]` header, then `key = value` lines until the next header, with
/// `#` comments and blank lines between. It fails unless exactly one entry
/// has the name, and that entry has a positive whole `period` and
/// `processor_time_bound`.
pub fn read(budgets: &str, name: &str) -> Result<Wake, String> {
    let mut entries = Vec::new();
    let mut current: Option<Vec<(String, String)>> = None;
    for line in budgets.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            entries.extend(current.take());
            if line == "[[wake]]" {
                current = Some(Vec::new());
            }
            continue;
        }
        if let Some(fields) = current.as_mut() {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("budgets.toml: a wake line is not `key = value`: {line}"))?;
            fields.push((key.trim().to_string(), value.trim().to_string()));
        }
    }
    entries.extend(current);

    let quoted = format!("\"{name}\"");
    let mut named = entries
        .iter()
        .filter(|fields| field(fields, "name") == Some(quoted.as_str()));
    let fields = named
        .next()
        .ok_or_else(|| format!("budgets.toml: no [[wake]] entry is named {quoted}"))?;
    if named.next().is_some() {
        return Err(format!(
            "budgets.toml: more than one [[wake]] entry is named {quoted}"
        ));
    }
    Ok(Wake {
        period_seconds: positive(fields, "period", name)?,
        processor_time_bound_ms: positive(fields, "processor_time_bound", name)?,
    })
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, value)| value.as_str())
}

fn positive(fields: &[(String, String)], key: &str, name: &str) -> Result<u64, String> {
    let value = field(fields, key)
        .ok_or_else(|| format!("budgets.toml: the {name:?} wake has no {key}"))?;
    match value.parse::<u64>() {
        Ok(number) if number > 0 => Ok(number),
        _ => Err(format!(
            "budgets.toml: the {name:?} wake's {key} is not a positive whole number: {value}"
        )),
    }
}

/// The line without a `#` comment, a `#` inside a string aside.
fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    for (index, ch) in line.char_indices() {
        match ch {
            '"' => in_string = !in_string,
            '#' if !in_string => return &line[..index],
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    const TWO: &str = r#"
# A comment, and a table that is not a wake.
[runners.tier1]
name = "update check"

[[wake]]
name = "update check"
period = 21600                 # seconds
processor_time_bound = 50      # ms
justifying_requirement = "FR-014"

[[wake]]
name = "blocking-list refresh"   # a # in a comment
period = 86400
processor_time_bound = 50
"#;

    #[test]
    fn reads_the_named_entry_alone() {
        assert_eq!(
            read(TWO, UPDATE_CHECK),
            Ok(Wake {
                period_seconds: 21600,
                processor_time_bound_ms: 50,
            })
        );
        assert_eq!(
            read(TWO, "blocking-list refresh").unwrap().period_seconds,
            86400
        );
    }

    #[test]
    fn the_file_in_this_repository_has_the_entry() {
        let budgets = include_str!("../../../../budgets.toml");
        let wake = read(budgets, UPDATE_CHECK).unwrap();
        assert!(wake.period_seconds > 0 && wake.processor_time_bound_ms > 0);
    }

    #[test]
    fn a_missing_duplicated_or_malformed_entry_is_refused() {
        assert!(read("", UPDATE_CHECK).unwrap_err().contains("no [[wake]]"));
        let twice = format!(
            "{TWO}\n[[wake]]\nname = \"update check\"\nperiod = 1\nprocessor_time_bound = 1\n"
        );
        assert!(
            read(&twice, UPDATE_CHECK)
                .unwrap_err()
                .contains("more than one")
        );
        for broken in [
            "[[wake]]\nname = \"update check\"\nprocessor_time_bound = 50\n",
            "[[wake]]\nname = \"update check\"\nperiod = 0\nprocessor_time_bound = 50\n",
            "[[wake]]\nname = \"update check\"\nperiod = 6h\nprocessor_time_bound = 50\n",
            "[[wake]]\nname = \"update check\"\nperiod = 60\n",
        ] {
            assert!(read(broken, UPDATE_CHECK).is_err(), "{broken}");
        }
    }
}
