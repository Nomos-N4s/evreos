//! FR-013 registration, run against an in-memory registry on every platform.
//!
//! Asserts that registering writes the browser's entry, its capabilities
//! naming a ProgID for `http`, `https`, `.htm` and `.html`, the two ProgIDs
//! and its `RegisteredApplications` value; that unregistering, as an
//! uninstall does, removes every one of them and nothing else; and that a
//! failed registration leaves nothing listed.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::io;

use evreos_platform::default_browser::{
    Application, InvalidApplication, REGISTERED_APPLICATIONS, Registration, Registry, register,
    unregister,
};

/// A registry held in memory: every key that exists, and every value, by
/// key and name. Key paths compare case-insensitively, as the system's do.
#[derive(Default)]
struct Memory {
    keys: BTreeMap<String, BTreeMap<String, String>>,
    /// Fails the write whose index this is, counting from zero.
    fail_at: Option<usize>,
    writes: usize,
    /// Fails every removal of this key, leaving it in place.
    stuck: Option<String>,
}

fn fold(key: &str) -> String {
    key.to_ascii_lowercase()
}

impl Memory {
    fn value(&self, key: &str, name: &str) -> Option<&str> {
        self.keys
            .get(&fold(key))?
            .get(&fold(name))
            .map(String::as_str)
    }

    fn has_key(&self, key: &str) -> bool {
        self.keys.contains_key(&fold(key))
    }
}

impl Registry for Memory {
    fn key_exists(&self, key: &str) -> io::Result<bool> {
        Ok(self.has_key(key))
    }

    fn set_string(&mut self, key: &str, name: &str, value: &str) -> io::Result<()> {
        let index = self.writes;
        self.writes += 1;
        if self.fail_at == Some(index) {
            return Err(io::Error::other("write refused"));
        }
        let mut path = String::new();
        for part in key.split('\\') {
            if !path.is_empty() {
                path.push('\\');
            }
            path.push_str(&fold(part));
            self.keys.entry(path.clone()).or_default();
        }
        self.keys
            .get_mut(&fold(key))
            .expect("created above")
            .insert(fold(name), value.to_string());
        Ok(())
    }

    fn remove_key(&mut self, key: &str) -> io::Result<()> {
        if self
            .stuck
            .as_deref()
            .is_some_and(|stuck| fold(stuck) == fold(key))
        {
            return Err(io::Error::other("removal refused"));
        }
        let key = fold(key);
        let below = format!("{key}\\");
        self.keys.retain(|k, _| *k != key && !k.starts_with(&below));
        Ok(())
    }

    fn remove_value(&mut self, key: &str, name: &str) -> io::Result<()> {
        if let Some(values) = self.keys.get_mut(&fold(key)) {
            values.remove(&fold(name));
        }
        Ok(())
    }
}

const APP: Application<'static> = Application {
    name: "Sample Browser",
    description: "A web browser.",
    executable: r"C:\Program Files\Sample\sample.exe",
};

const CLIENT: &str = r"Software\Clients\StartMenuInternet\SampleBrowser";
const CAPABILITIES: &str = r"Software\Clients\StartMenuInternet\SampleBrowser\Capabilities";

/// Another browser's registration, and the member's own file association,
/// which neither call may touch.
fn with_neighbours() -> Memory {
    let mut registry = Memory::default();
    for (key, name, value) in [
        (r"Software\Clients\StartMenuInternet\Other", "", "Other"),
        (
            REGISTERED_APPLICATIONS,
            "Other",
            r"Software\Clients\StartMenuInternet\Other\Capabilities",
        ),
        (r"Software\Classes\OtherHTML", "", "Other"),
        (r"Software\Classes\.html", "", "OtherHTML"),
    ] {
        registry.set_string(key, name, value).unwrap();
    }
    registry.writes = 0;
    registry
}

#[test]
fn registering_writes_the_browser_entry_and_its_capabilities() {
    let mut registry = with_neighbours();
    register(&mut registry, &APP).unwrap();

    let exe = r#""C:\Program Files\Sample\sample.exe""#;
    let icon = format!("{exe},0");
    let open = format!(r#"{exe} "%1""#);
    let expected: &[(&str, &str, &str)] = &[
        (CLIENT, "", "Sample Browser"),
        (CAPABILITIES, "ApplicationName", "Sample Browser"),
        (CAPABILITIES, "ApplicationDescription", "A web browser."),
        (CAPABILITIES, "ApplicationIcon", &icon),
        (
            &format!(r"{CAPABILITIES}\FileAssociations"),
            ".htm",
            "SampleBrowser.HTML",
        ),
        (
            &format!(r"{CAPABILITIES}\FileAssociations"),
            ".html",
            "SampleBrowser.HTML",
        ),
        (
            &format!(r"{CAPABILITIES}\URLAssociations"),
            "http",
            "SampleBrowser.URL",
        ),
        (
            &format!(r"{CAPABILITIES}\URLAssociations"),
            "https",
            "SampleBrowser.URL",
        ),
        (
            &format!(r"{CAPABILITIES}\StartMenu"),
            "StartMenuInternet",
            "SampleBrowser",
        ),
        (&format!(r"{CLIENT}\DefaultIcon"), "", &icon),
        (&format!(r"{CLIENT}\shell\open\command"), "", exe),
        (r"Software\Classes\SampleBrowser.HTML", "", "Sample Browser"),
        (
            r"Software\Classes\SampleBrowser.HTML\DefaultIcon",
            "",
            &icon,
        ),
        (
            r"Software\Classes\SampleBrowser.HTML\shell\open\command",
            "",
            &open,
        ),
        (r"Software\Classes\SampleBrowser.URL", "", "Sample Browser"),
        (r"Software\Classes\SampleBrowser.URL", "URL Protocol", ""),
        (r"Software\Classes\SampleBrowser.URL\DefaultIcon", "", &icon),
        (
            r"Software\Classes\SampleBrowser.URL\shell\open\command",
            "",
            &open,
        ),
        (REGISTERED_APPLICATIONS, "Sample Browser", CAPABILITIES),
    ];
    for (key, name, value) in expected {
        assert_eq!(registry.value(key, name), Some(*value), "{key} [{name}]");
    }
    assert_eq!(
        Registration::of(&APP).unwrap().values.len(),
        expected.len(),
        "registration writes a value this test does not name"
    );
    assert_eq!(
        registry.value(r"Software\Classes\.html", ""),
        Some("OtherHTML")
    );
}

#[test]
fn the_registered_applications_value_is_written_last() {
    let registration = Registration::of(&APP).unwrap();
    let last = registration.values.last().unwrap();
    assert_eq!(
        (last.key.as_str(), last.name.as_str()),
        (REGISTERED_APPLICATIONS, "Sample Browser")
    );
}

#[test]
fn unregistering_removes_every_key_and_value_and_nothing_else() {
    let untouched = with_neighbours();
    let mut registry = with_neighbours();
    register(&mut registry, &APP).unwrap();
    unregister(&mut registry, &APP).unwrap();

    for key in [
        CLIENT,
        r"Software\Classes\SampleBrowser.HTML",
        r"Software\Classes\SampleBrowser.URL",
    ] {
        assert!(!registry.has_key(key), "{key} survived the uninstall");
    }
    assert_eq!(
        registry.value(REGISTERED_APPLICATIONS, "Sample Browser"),
        None
    );
    assert_eq!(registry.keys, untouched.keys);
}

#[test]
fn unregistering_twice_or_before_registering_is_not_an_error() {
    let mut registry = with_neighbours();
    unregister(&mut registry, &APP).unwrap();
    register(&mut registry, &APP).unwrap();
    unregister(&mut registry, &APP).unwrap();
    unregister(&mut registry, &APP).unwrap();
    assert_eq!(registry.keys, with_neighbours().keys);
}

#[test]
fn a_failed_registration_leaves_nothing_listed() {
    let count = Registration::of(&APP).unwrap().values.len();
    for fail_at in 0..count {
        let mut registry = with_neighbours();
        registry.fail_at = Some(fail_at);
        let error = register(&mut registry, &APP).unwrap_err();
        assert_eq!(error.to_string(), "write refused");
        assert_eq!(
            registry.keys,
            with_neighbours().keys,
            "failing write {fail_at}"
        );
    }
}

#[test]
fn keys_and_progids_come_from_the_name_alone() {
    let app = Application {
        name: "Ünïcode Browser 2",
        ..APP
    };
    let registration = Registration::of(&app).unwrap();
    assert_eq!(registration.key, "ncodeBrowser2");
    assert_eq!(
        registration.owned_keys,
        [
            r"Software\Clients\StartMenuInternet\ncodeBrowser2",
            r"Software\Classes\ncodeBrowser2.HTML",
            r"Software\Classes\ncodeBrowser2.URL",
        ]
    );
    // The name is still shown as given.
    assert_eq!(registration.values[0].data, "Ünïcode Browser 2");
}

#[test]
fn an_application_that_cannot_be_registered_writes_nothing() {
    let long = "x".repeat(35);
    let cases: &[(Application<'_>, InvalidApplication)] = &[
        (Application { name: "", ..APP }, InvalidApplication::Name),
        (
            Application {
                name: "Sample\tBrowser",
                ..APP
            },
            InvalidApplication::Name,
        ),
        (
            Application {
                name: "7 Browser",
                ..APP
            },
            InvalidApplication::NameWithoutKey,
        ),
        (
            Application {
                name: "ÄÖÜ -",
                ..APP
            },
            InvalidApplication::NameWithoutKey,
        ),
        (
            Application { name: &long, ..APP },
            InvalidApplication::NameTooLong,
        ),
        (
            Application {
                description: "",
                ..APP
            },
            InvalidApplication::Description,
        ),
        (
            Application {
                description: "a\nb",
                ..APP
            },
            InvalidApplication::Description,
        ),
        (
            Application {
                executable: "sample.exe",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: r"\sample.exe",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: r"C:sample.exe",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: "/usr/bin/sample",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: r"\\?\C:\sample.exe",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: r"\\.\pipe\sample",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: r"C:\x%1y\sample.exe",
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: r#"C:\a"b\sample.exe"#,
                ..APP
            },
            InvalidApplication::Executable,
        ),
        (
            Application {
                executable: "C:\\a\tb\\sample.exe",
                ..APP
            },
            InvalidApplication::Executable,
        ),
    ];
    for (app, invalid) in cases {
        let mut registry = with_neighbours();
        let error = register(&mut registry, app).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{app:?}");
        assert_eq!(Registration::of(app).unwrap_err(), *invalid, "{app:?}");
        assert_eq!(registry.writes, 0, "{app:?}");
        assert!(unregister(&mut registry, app).is_err(), "{app:?}");
    }
    // The longest name that fits: 34 characters and a five-character suffix.
    let longest = "x".repeat(34);
    Registration::of(&Application {
        name: &longest,
        ..APP
    })
    .unwrap();
    // A share path is absolute.
    Registration::of(&Application {
        executable: r"\\server\share\sample.exe",
        ..APP
    })
    .unwrap();
}

/// The tier-1 binding, against the real registry: registering under a
/// scratch key below the current user's hive writes every value the
/// registration names, as a string, and unregistering removes every key
/// and value it wrote. The scratch key stands in for the hive, so the run
/// lists no browser on the machine it runs on.
#[cfg(windows)]
#[test]
fn the_windows_registry_holds_the_registration_and_loses_it_on_uninstall() {
    use evreos_platform::default_browser::WindowsRegistry;
    use windows_registry::CURRENT_USER;

    const SCRATCH_PARENT: &str = r"Software\PlatformRegistrationTest";
    let scratch = format!(
        r"{SCRATCH_PARENT}\{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut registry = WindowsRegistry::below_current_user(&scratch).unwrap();
    let root = CURRENT_USER.open(&scratch).unwrap();
    root.create(REGISTERED_APPLICATIONS)
        .unwrap()
        .set_string("Other", "kept")
        .unwrap();

    register(&mut registry, &APP).unwrap();
    let registration = Registration::of(&APP).unwrap();
    for value in &registration.values {
        let key = root.open(&value.key).unwrap();
        assert_eq!(
            key.get_type(&value.name).unwrap(),
            windows_registry::Type::String,
            "{} [{}]",
            value.key,
            value.name
        );
        assert_eq!(key.get_string(&value.name).unwrap(), value.data);
    }

    for key in &registration.owned_keys {
        assert!(registry.key_exists(key).unwrap(), "{key}");
    }

    unregister(&mut registry, &APP).unwrap();
    for key in &registration.owned_keys {
        assert!(root.open(key).is_err(), "{key} survived the uninstall");
        assert!(!registry.key_exists(key).unwrap(), "{key}");
    }
    let registered = root.open(REGISTERED_APPLICATIONS).unwrap();
    assert!(registered.get_string("Sample Browser").is_err());
    assert_eq!(registered.get_string("Other").unwrap(), "kept");
    // Removing again, and removing a value under a key that is gone, are
    // not errors.
    unregister(&mut registry, &APP).unwrap();
    registry
        .remove_value(r"Software\Absent\Key", "value")
        .unwrap();

    drop(registered);
    drop(root);
    CURRENT_USER.remove_tree(&scratch).unwrap();
    assert!(CURRENT_USER.open(&scratch).is_err());
    // The parent too, once no other run's scratch key is left under it.
    let parent = CURRENT_USER.open(SCRATCH_PARENT).unwrap();
    if parent.keys().unwrap().next().is_none() {
        drop(parent);
        CURRENT_USER.remove_tree(SCRATCH_PARENT).unwrap();
    }
}

/// The binding opens the current user's hive itself, where registration
/// writes, without writing anything.
#[cfg(windows)]
#[test]
fn the_windows_registry_opens_the_current_users_hive() {
    evreos_platform::default_browser::WindowsRegistry::current_user().unwrap();
}

#[test]
fn tier_one_registers_then_opens_the_default_apps_page() {
    use evreos_platform::default_browser::{Route, SETTINGS_PAGE, open_settings, route};

    assert_eq!(SETTINGS_PAGE, "ms-settings:defaultapps");
    if cfg!(windows) {
        assert_eq!(route(), Route::RegisterThenSettings);
    } else {
        // Every other platform, tier 2 among them, has no route yet, and
        // says so rather than open anything.
        assert_eq!(route(), Route::Unestablished);
        assert_eq!(
            open_settings().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }
}

/// The system's launcher is reached and takes the page's address on Windows
/// itself. The launch is not awaited, so whether the page opens is not
/// checked here. It opens the default-apps page on the machine running it,
/// so it runs only under CI, whose Windows runner is ephemeral, and passes
/// without launching anything elsewhere.
#[cfg(windows)]
#[test]
fn the_windows_launcher_takes_the_default_apps_page() {
    if std::env::var_os("CI").is_none() {
        eprintln!("skipped outside CI: it would open the system's settings");
        return;
    }
    evreos_platform::default_browser::open_settings().unwrap();
}

#[test]
fn a_removal_that_fails_does_not_stop_the_others() {
    let mut registry = with_neighbours();
    register(&mut registry, &APP).unwrap();
    registry.stuck = Some(CLIENT.to_string());

    let error = unregister(&mut registry, &APP).unwrap_err();
    assert_eq!(error.to_string(), "removal refused");
    assert!(registry.has_key(CLIENT));
    for key in [
        r"Software\Classes\SampleBrowser.HTML",
        r"Software\Classes\SampleBrowser.URL",
    ] {
        assert!(!registry.has_key(key), "{key} was left behind");
    }
    assert_eq!(
        registry.value(REGISTERED_APPLICATIONS, "Sample Browser"),
        None
    );
}

#[test]
fn a_failed_re_registration_leaves_the_existing_one_listed() {
    let count = Registration::of(&APP).unwrap().values.len();
    for fail_at in 0..count {
        let mut registry = with_neighbours();
        register(&mut registry, &APP).unwrap();
        let registered = registry.keys.clone();
        registry.writes = 0;
        registry.fail_at = Some(fail_at);

        let error = register(&mut registry, &APP).unwrap_err();
        assert_eq!(error.to_string(), "write refused");
        // The same application re-registered writes the same values, so
        // nothing it had is lost.
        assert_eq!(registry.keys, registered, "failing write {fail_at}");
    }
}

#[test]
fn a_failed_first_registration_keeps_a_key_it_did_not_create() {
    const FOREIGN: &str = r"Software\Classes\SampleBrowser.URL";
    let count = Registration::of(&APP).unwrap().values.len();
    for fail_at in 0..count {
        let mut registry = with_neighbours();
        registry.set_string(FOREIGN, "", "foreign").unwrap();
        registry.writes = 0;
        registry.fail_at = Some(fail_at);

        register(&mut registry, &APP).unwrap_err();
        assert!(!registry.has_key(CLIENT), "failing write {fail_at}");
        assert!(
            !registry.has_key(r"Software\Classes\SampleBrowser.HTML"),
            "failing write {fail_at}"
        );
        assert_eq!(
            registry.value(REGISTERED_APPLICATIONS, "Sample Browser"),
            None,
            "failing write {fail_at}"
        );
        assert!(registry.has_key(FOREIGN), "failing write {fail_at}");
    }
}
