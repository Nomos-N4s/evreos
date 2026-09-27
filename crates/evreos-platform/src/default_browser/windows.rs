//! The tier-1 [`Registry`]: the current user's hive, through the
//! `windows-registry` binding.

use std::io;

use windows_registry::{CURRENT_USER, Key, OpenOptions};

use super::Registry;

/// The `DELETE` standard access right, which removing a key's tree needs on
/// the key it is removed from, and which `KEY_READ | KEY_WRITE` lacks.
const DELETE: u32 = 0x0001_0000;
/// `ERROR_FILE_NOT_FOUND` as the binding reports it, an `HRESULT` in the
/// Win32 facility: what a missing key or value returns.
const NOT_FOUND: i32 = 0x8007_0002_u32 as i32;

/// The current user's hive, or a key below it that stands in for it.
#[derive(Debug)]
pub struct WindowsRegistry {
    root: Key,
}

impl WindowsRegistry {
    /// The current user's hive, where [`register`](super::register) writes.
    pub fn current_user() -> io::Result<Self> {
        Self::open(CURRENT_USER.options().read().write().access(DELETE), "")
    }

    /// A key below the current user's hive, created if absent, that every
    /// key path is taken relative to. A test registers under one so that it
    /// exercises this binding without listing a browser on the machine it
    /// runs on.
    pub fn below_current_user(path: &str) -> io::Result<Self> {
        Self::open(
            CURRENT_USER
                .options()
                .read()
                .write()
                .access(DELETE)
                .create(),
            path,
        )
    }

    fn open(options: &OpenOptions<'_>, path: &str) -> io::Result<Self> {
        Ok(Self {
            root: options.open(path)?,
        })
    }
}

impl Registry for WindowsRegistry {
    fn key_exists(&self, key: &str) -> io::Result<bool> {
        match self.root.open(key) {
            Ok(_) => Ok(true),
            Err(error) if error.code().0 == NOT_FOUND => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn set_string(&mut self, key: &str, name: &str, value: &str) -> io::Result<()> {
        Ok(self.root.create(key)?.set_string(name, value)?)
    }

    fn remove_key(&mut self, key: &str) -> io::Result<()> {
        absent_is_done(self.root.remove_tree(key))
    }

    fn string_values(&self, key: &str) -> io::Result<Vec<(String, String)>> {
        let key = match self.root.open(key) {
            Ok(key) => key,
            Err(error) if error.code().0 == NOT_FOUND => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        Ok(key
            .values()?
            .filter_map(|(name, value)| Some((name, String::try_from(value).ok()?)))
            .collect())
    }

    fn remove_value(&mut self, key: &str, name: &str) -> io::Result<()> {
        match self.root.options().write().open(key) {
            Ok(key) => absent_is_done(key.remove_value(name)),
            Err(error) if error.code().0 == NOT_FOUND => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

/// A removal whose target is already gone has done what it was asked.
fn absent_is_done(result: windows_registry::Result<()>) -> io::Result<()> {
    match result {
        Err(error) if error.code().0 == NOT_FOUND => Ok(()),
        other => Ok(other?),
    }
}
