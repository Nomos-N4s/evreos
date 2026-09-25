// Logging a Sensitive<T> wrapper directly must fail to typecheck (FR-023).
use evreos_shell::log::{EventKind, Level, Record, Sensitive};

fn main() {
    let sensitive = Sensitive::new("raw-sensitive-value");
    let _ = Record::builder(Level::Info, EventKind::Lifecycle)
        .field("secret", sensitive)
        .build();
}
