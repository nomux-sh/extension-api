//! Typed access to the keys of one TOML table.

use toml::{Table, Value};

use super::error::Problems;

/// One table of the manifest. Each read removes its key. The keys that stay
/// until [`Fields::finish`] are not known.
pub(crate) struct Fields {
    path: String,
    table: Table,
}

impl Fields {
    pub(crate) fn root(table: Table) -> Self {
        Self {
            path: String::new(),
            table,
        }
    }

    /// The path of a key of this table from the root of the manifest.
    pub(crate) fn path(&self, key: &str) -> String {
        if self.path.is_empty() {
            return key.to_owned();
        }
        format!("{}.{key}", self.path)
    }

    /// Reads a string that the table must have.
    pub(crate) fn string(&mut self, key: &str, problems: &mut Problems) -> Option<String> {
        match self.table.remove(key) {
            Some(Value::String(text)) => Some(text),
            Some(_) => {
                problems.add(self.path(key), "must be a string");
                None
            }
            None => {
                problems.add(self.path(key), "is missing");
                None
            }
        }
    }

    /// Reads a string that the table must have and that must obey a rule.
    pub(crate) fn string_matching(
        &mut self,
        key: &str,
        is_valid: impl Fn(&str) -> bool,
        rule: &str,
        problems: &mut Problems,
    ) -> Option<String> {
        let text = self.string(key, problems)?;
        if !is_valid(&text) {
            problems.add(self.path(key), rule);
            return None;
        }
        Some(text)
    }

    /// Reads a boolean. The result is `default` if the table does not have
    /// the key.
    pub(crate) fn boolean(&mut self, key: &str, default: bool, problems: &mut Problems) -> bool {
        match self.table.remove(key) {
            Some(Value::Boolean(value)) => value,
            Some(_) => {
                problems.add(self.path(key), "must be `true` or `false`");
                default
            }
            None => default,
        }
    }

    /// Reads a list of strings in which each item must obey a rule. The
    /// result is empty if the table does not have the key.
    pub(crate) fn strings_matching(
        &mut self,
        key: &str,
        is_valid: impl Fn(&str) -> bool,
        rule: &str,
        problems: &mut Problems,
    ) -> Vec<String> {
        let items = match self.table.remove(key) {
            Some(Value::Array(items)) => items,
            Some(_) => {
                problems.add(self.path(key), "must be a list of strings");
                return Vec::new();
            }
            None => return Vec::new(),
        };
        let mut texts = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            let path = format!("{}[{index}]", self.path(key));
            match item {
                Value::String(text) if is_valid(&text) => texts.push(text),
                Value::String(_) => problems.add(path, rule),
                _ => problems.add(path, "must be a string"),
            }
        }
        texts
    }

    /// Reads a table that the manifest can omit.
    pub(crate) fn table(&mut self, key: &str, problems: &mut Problems) -> Option<Self> {
        match self.table.remove(key)? {
            Value::Table(table) => Some(Self {
                path: self.path(key),
                table,
            }),
            _ => {
                problems.add(self.path(key), "must be a table");
                None
            }
        }
    }

    /// Reads a table that the manifest must have.
    pub(crate) fn required_table(&mut self, key: &str, problems: &mut Problems) -> Option<Self> {
        if !self.table.contains_key(key) {
            problems.add(self.path(key), "is missing");
            return None;
        }
        self.table(key, problems)
    }

    /// Reports each key that no read asked for.
    pub(crate) fn finish(self, problems: &mut Problems) {
        for key in self.table.keys() {
            problems.add(self.path(key), "is not a known key");
        }
    }
}
