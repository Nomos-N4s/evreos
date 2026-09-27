//! FR-013: making the browser the member's default from within it.
//!
//! Tier 1 (Windows) does not let a third-party browser make itself the
//! default. The most a browser can do there is register itself so that it
//! appears in the system's list of browsers at all, and then open the
//! system's default-apps page, where the member makes the choice (research
//! §10.2). This module does both: [`register`], then [`open_settings`].
//! [`route`] says which platforms take that route.
//!
//! **Tier 2** (macOS) has no route here. Documented calls exist, but what
//! they do at the macOS 13 floor is unverified until it is established on the
//! tier-2 pinned runner, as `docs/measurements/n12-default-browser-macos.md`
//! records; until then [`route`] reports it unestablished rather than assume
//! an API exists.
//!
//! **Registration** is a fixed set of string values under the current user's
//! hive: the browser's entry under `Software\Clients\StartMenuInternet`, its
//! `Capabilities` naming a ProgID for `http`, `https`, `.htm` and `.html`,
//! the two ProgIDs under `Software\Classes`, and one value under
//! `Software\RegisteredApplications` pointing at those capabilities. It is
//! written per user, so it needs no elevation. [`register`] writes it and
//! [`unregister`] removes it: the three keys this module owns, whole, and
//! its one value under `RegisteredApplications`, which other applications
//! share. Neither touches anything else, so an uninstall leaves the
//! member's own choice of default, and every other browser's registration,
//! as it found them.
//!
//! Microsoft's guidance also has a browser announce a new registration by
//! calling `SHChangeNotify` with `SHCNE_ASSOCCHANGED`, so that the shell
//! refreshes what it has cached. That call is reachable from Rust only as an
//! `unsafe` function, and this crate forbids `unsafe` code, so it is not
//! made. Whether the system lists the browser without it, on Windows 10 and
//! 11, is owed to the tier-1 check that
//! `docs/measurements/default-browser-registration.md` lists.
//!
//! Both go through [`Registry`], so the set of values is decided here, on
//! every platform, and tested on every platform; a platform binding only
//! stores strings. The tier-1 binding is `WindowsRegistry`, built on Windows
//! alone.
//!
//! No brand name appears here (FR-042). The product name reaches this module
//! as [`Application::name`], from the shell's brand configuration, and every
//! key and ProgID is derived from it.

use std::fmt;
use std::io;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::WindowsRegistry;

/// The browser being registered.
#[derive(Clone, Copy, Debug)]
pub struct Application<'a> {
    /// The product name, from the brand configuration. Shown in the system's
    /// list of browsers, and the source of every key and ProgID.
    pub name: &'a str,
    /// One sentence the system may show beside the name, already in the
    /// member's language.
    pub description: &'a str,
    /// The absolute path of the browser's executable.
    pub executable: &'a str,
}

/// Why an [`Application`] cannot be registered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidApplication {
    /// The name is empty, starts with `@`, or holds a control character.
    Name,
    /// The name's ASCII letters and digits, from which the key is derived,
    /// are none, or start with a digit, which a ProgID may not.
    NameWithoutKey,
    /// The key derived from the name is longer than a ProgID allows.
    NameTooLong,
    /// The description is empty, starts with `@`, or holds a control
    /// character.
    Description,
    /// The executable is not an absolute Windows path, or holds a quote, a
    /// `%` or a control character. A quote or a control character would
    /// break the command line, and a `%` could be read as the `%1` Windows
    /// replaces with the address being opened.
    Executable,
}

impl fmt::Display for InvalidApplication {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Name => "the name is empty, starts with @ or holds a control character",
            Self::NameWithoutKey => "the name yields no key that starts with an ASCII letter",
            Self::NameTooLong => "the key derived from the name is too long",
            Self::Description => {
                "the description is empty, starts with @ or holds a control character"
            }
            Self::Executable => "the executable is not a plain absolute path",
        })
    }
}

impl std::error::Error for InvalidApplication {}

impl From<InvalidApplication> for io::Error {
    fn from(invalid: InvalidApplication) -> Self {
        io::Error::new(io::ErrorKind::InvalidInput, invalid)
    }
}

/// A store of string values under the current user's hive, addressed by a
/// key path below it with `\` between its parts.
///
/// An empty value name is the key's default value.
pub trait Registry {
    /// Whether a key exists.
    fn key_exists(&self, key: &str) -> io::Result<bool>;
    /// Sets a string value, creating the key and every key above it.
    fn set_string(&mut self, key: &str, name: &str, value: &str) -> io::Result<()>;
    /// Removes a key with everything below it. A key that does not exist is
    /// not an error.
    fn remove_key(&mut self, key: &str) -> io::Result<()>;
    /// The string values a key holds, by name. A key that does not exist
    /// holds none.
    fn string_values(&self, key: &str) -> io::Result<Vec<(String, String)>>;
    /// Removes one value, leaving its key and the key's other values. A key
    /// or value that does not exist is not an error.
    fn remove_value(&mut self, key: &str, name: &str) -> io::Result<()>;
}

/// The key under which applications register their capabilities.
pub const REGISTERED_APPLICATIONS: &str = r"Software\RegisteredApplications";

/// The longest a ProgID may be, in characters.
const PROGID_MAX: usize = 39;
/// The suffixes this module appends to the key to name its two ProgIDs, in
/// the `Application.Component` form ProgIDs take.
const HTML_SUFFIX: &str = ".HTML";
const URL_SUFFIX: &str = ".URL";

/// One string value [`register`] writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Value {
    /// The key path below the current user's hive.
    pub key: String,
    /// The value name; empty for the key's default value.
    pub name: String,
    /// The string stored.
    pub data: String,
}

/// Everything registration writes, and what removing it deletes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    /// The product name, as shown. It names the registration's value under
    /// [`REGISTERED_APPLICATIONS`], which Windows requires to match the
    /// `ApplicationName` among its capabilities.
    pub name: String,
    /// The name the registration's keys are filed under: the product name's
    /// ASCII letters and digits, in order. Two names that differ only in
    /// other characters share it, and so share one registration.
    pub key: String,
    /// Every value written, in the order [`register`] writes them.
    pub values: Vec<Value>,
    /// The keys this registration owns, which [`unregister`] removes whole.
    pub owned_keys: Vec<String>,
    /// The capabilities key, which the value under
    /// [`REGISTERED_APPLICATIONS`] points at.
    pub capabilities: String,
}

impl Registration {
    /// The registration for `app`, checked before anything is written.
    pub fn of(app: &Application<'_>) -> Result<Self, InvalidApplication> {
        if !is_plain_text(app.name) {
            return Err(InvalidApplication::Name);
        }
        let key: String = app
            .name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .collect();
        if !key.starts_with(|ch: char| ch.is_ascii_alphabetic()) {
            return Err(InvalidApplication::NameWithoutKey);
        }
        if key.len() + HTML_SUFFIX.len().max(URL_SUFFIX.len()) > PROGID_MAX {
            return Err(InvalidApplication::NameTooLong);
        }
        if !is_plain_text(app.description) {
            return Err(InvalidApplication::Description);
        }
        if !is_plain_absolute(app.executable) {
            return Err(InvalidApplication::Executable);
        }

        let client = format!(r"Software\Clients\StartMenuInternet\{key}");
        let capabilities = format!(r"{client}\Capabilities");
        let html = format!("{key}{HTML_SUFFIX}");
        let url = format!("{key}{URL_SUFFIX}");
        let html_class = format!(r"Software\Classes\{html}");
        let url_class = format!(r"Software\Classes\{url}");
        let icon = format!("\"{}\",0", app.executable);
        let launch = format!("\"{}\"", app.executable);
        let open = format!("\"{}\" \"%1\"", app.executable);

        let mut values = Vec::new();
        let mut set = |key: &str, name: &str, data: &str| {
            values.push(Value {
                key: key.to_string(),
                name: name.to_string(),
                data: data.to_string(),
            });
        };
        set(&client, "", app.name);
        set(&capabilities, "ApplicationName", app.name);
        set(&capabilities, "ApplicationDescription", app.description);
        set(&capabilities, "ApplicationIcon", &icon);
        let files = format!(r"{capabilities}\FileAssociations");
        set(&files, ".htm", &html);
        set(&files, ".html", &html);
        let urls = format!(r"{capabilities}\URLAssociations");
        set(&urls, "http", &url);
        set(&urls, "https", &url);
        set(
            &format!(r"{capabilities}\StartMenu"),
            "StartMenuInternet",
            &key,
        );
        set(&format!(r"{client}\DefaultIcon"), "", &icon);
        set(&format!(r"{client}\shell\open\command"), "", &launch);
        for class in [&html_class, &url_class] {
            set(class, "", app.name);
            set(&format!(r"{class}\DefaultIcon"), "", &icon);
            set(&format!(r"{class}\shell\open\command"), "", &open);
        }
        set(&url_class, "URL Protocol", "");
        // Last, so the system never lists capabilities that are not yet all
        // written.
        set(REGISTERED_APPLICATIONS, app.name, &capabilities);

        Ok(Self {
            name: app.name.to_string(),
            key,
            values,
            owned_keys: vec![client, html_class, url_class],
            capabilities,
        })
    }
}

/// Registers `app` so that the system lists it as a browser.
///
/// If a write fails, the write's error is returned. A first registration,
/// one whose entry under `StartMenuInternet` did not exist, is then undone:
/// its value under [`REGISTERED_APPLICATIONS`] is removed, and so is every
/// key it owns that it created, so nothing is listed. A key it owns that
/// existed before, a ProgID of the same name say, keeps its place, with the
/// values written before the failure in it. A registration that was already
/// there, which the member may have chosen as their default, is left in
/// place: the values written before the failure replace their earlier
/// copies, and the rest keep theirs.
pub fn register(registry: &mut impl Registry, app: &Application<'_>) -> io::Result<()> {
    let registration = Registration::of(app)?;
    let mut existed = Vec::with_capacity(registration.owned_keys.len());
    for key in &registration.owned_keys {
        existed.push(registry.key_exists(key)?);
    }
    // The entry under StartMenuInternet is the first owned key.
    let first = !existed[0];
    for value in &registration.values {
        if let Err(error) = registry.set_string(&value.key, &value.name, &value.data) {
            if first {
                let _ = registry.remove_value(REGISTERED_APPLICATIONS, &registration.name);
                for (key, existed) in registration.owned_keys.iter().zip(&existed) {
                    if !existed {
                        let _ = registry.remove_key(key);
                    }
                }
            }
            return Err(error);
        }
    }
    // A registration under an earlier name with the same key points at the
    // same capabilities from a value of its own name, which no longer
    // matches ApplicationName; only this registration's value may remain.
    remove_pointers(registry, &registration, Some(&registration.name))
}

/// Removes what [`register`] wrote for `app`, as an uninstall must.
///
/// Unregistering an application that was never registered, or unregistering
/// it twice, is not an error.
pub fn unregister(registry: &mut impl Registry, app: &Application<'_>) -> io::Result<()> {
    remove(registry, &Registration::of(app)?)
}

fn remove(registry: &mut impl Registry, registration: &Registration) -> io::Result<()> {
    // The pointer first, so the system never lists capabilities that are
    // half removed. Every removal is tried even after one fails, so a
    // failure leaves as little behind as it can, and the first error is
    // the one reported.
    let mut result = registry
        .remove_value(REGISTERED_APPLICATIONS, &registration.name)
        .and_then(|()| remove_pointers(registry, registration, None));
    for key in &registration.owned_keys {
        let removed = registry.remove_key(key);
        if result.is_ok() {
            result = removed;
        }
    }
    result
}

/// Removes every value under [`REGISTERED_APPLICATIONS`] that points at this
/// registration's capabilities, but the one named `keep`.
fn remove_pointers(
    registry: &mut impl Registry,
    registration: &Registration,
    keep: Option<&str>,
) -> io::Result<()> {
    for (name, data) in registry.string_values(REGISTERED_APPLICATIONS)? {
        let kept = keep.is_some_and(|keep| keep.eq_ignore_ascii_case(&name));
        if !kept && data.eq_ignore_ascii_case(&registration.capabilities) {
            registry.remove_value(REGISTERED_APPLICATIONS, &name)?;
        }
    }
    Ok(())
}

/// Whether `text` is shown as written: not empty, holding no control
/// character, and not starting with `@`, which Windows reads as a reference
/// to a string in a resource file rather than as the string itself.
fn is_plain_text(text: &str) -> bool {
    !text.is_empty() && !text.starts_with('@') && !text.chars().any(char::is_control)
}

/// Whether `path` is an absolute Windows path that can sit between quotes on
/// a command line: on a drive (`C:\`), or on a share (`\\server\`) whose
/// server name starts with a letter or digit, so never a `\\?\` or `\\.\`
/// device path; and holding no `%`, which Windows would read as the start of
/// a placeholder such as `%1`.
fn is_plain_absolute(path: &str) -> bool {
    let rooted = match path.as_bytes() {
        [drive, b':', b'\\', ..] => drive.is_ascii_alphabetic(),
        [b'\\', b'\\', server, ..] => server.is_ascii_alphanumeric(),
        _ => false,
    };
    rooted
        && !path
            .chars()
            .any(|ch| ch == '"' || ch == '%' || ch.is_control())
}

/// How FR-013 is met on the platform this was built for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// [`register`], then [`open_settings`] for the member to choose.
    RegisterThenSettings,
    /// No route is established for this platform.
    Unestablished,
}

/// The route FR-013 takes on this platform.
pub const fn route() -> Route {
    if cfg!(windows) {
        Route::RegisterThenSettings
    } else {
        Route::Unestablished
    }
}

/// The tier-1 system page where the member chooses the default browser.
pub const SETTINGS_PAGE: &str = "ms-settings:defaultapps";

/// Asks the system to open [`SETTINGS_PAGE`], after [`register`], for the
/// member to choose the browser there. The page is the system's, and the
/// browser cannot choose for them.
///
/// The page is handed to the system's launcher, and this returns once the
/// launcher has taken the request, without waiting for it, so it never
/// blocks the thread that calls it. An error here means the launcher could
/// not be reached or the page's address was refused. Whether the page then
/// opened is reported only by the launcher's own asynchronous result, which
/// this does not observe: the launcher declines, for one, when no window of
/// the calling application is visible, so a surface calls this from a
/// visible window. On a platform whose [`route`] is not
/// [`Route::RegisterThenSettings`] it returns [`io::ErrorKind::Unsupported`].
pub fn open_settings() -> io::Result<()> {
    #[cfg(windows)]
    {
        use ::windows::Foundation::Uri;
        use ::windows::System::Launcher;

        let page = Uri::CreateUri(&SETTINGS_PAGE.into())?;
        Launcher::LaunchUriAsync(&page)?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no default-browser route is established on this platform",
        ))
    }
}
