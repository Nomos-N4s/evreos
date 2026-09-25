// Logging an address must fail to typecheck (FR-007a, FR-023, FR-039c).
use evreos_shell::log::{Address, EventKind, Level, Record};

fn main() {
    let address = Address::new("https://sensitive.example.com");
    let _ = Record::builder(Level::Info, EventKind::Navigation)
        .field("address", address)
        .build();
}
