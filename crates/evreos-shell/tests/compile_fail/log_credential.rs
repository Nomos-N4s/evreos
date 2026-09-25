// Logging a credential must fail to typecheck (FR-023).
use evreos_shell::log::{Credential, EventKind, Level, Record};

fn main() {
    let credential = Credential::new("supersecretpassword");
    let _ = Record::builder(Level::Info, EventKind::Lifecycle)
        .field("credential", credential)
        .build();
}
