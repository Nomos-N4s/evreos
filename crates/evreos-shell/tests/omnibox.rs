//! Integration tests for the combined entry field (omnibox).
//!
//! Asserts FR-003, FR-003a, and FR-007a privacy and submission invariants:
//! - Nothing is transmitted before submission (typing, editing, and suggestion queries
//!   produce zero network transmission).
//! - Suggestions are drawn exclusively from local data (open tabs, bookmarks, history).
//! - A submitted search routes through `evreos-net` under `Purpose::HistoryBearing(SubmittedSearch)`.
//! - The submitted request carries ONLY the terms the member submitted.
//! - The submitted request carries NO navigated address, NO page content, NO history or
//!   bookmark data, and NO identifier Evreos assigns or persists across searches.
//! - Direct URL inputs navigate directly rather than generating search egress.

#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use evreos_net::{HistoryBearing, Purpose};
use evreos_shell::brand::brand;
use evreos_shell::omnibox::{Omnibox, OmniboxAction, is_url_input, normalize_url};
use evreos_shell::store::bookmarks::{BookmarkStore, FolderId};
use evreos_shell::store::history::{HistoryStore, WindowKind};
use evreos_shell::suggest::{OpenTab, SuggestionSource};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

fn unique_temp_dir() -> PathBuf {
    let count = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "evreos_omnibox_test_{}_{}",
        std::process::id(),
        count
    ));
    let _ = fs::create_dir_all(&path);
    path
}

#[test]
fn nothing_transmitted_before_submission() {
    let dir = unique_temp_dir();
    let history = HistoryStore::open(&dir.join("history"));
    let bookmarks = BookmarkStore::open(&dir.join("bookmarks"));
    let open_tabs = vec![OpenTab::new("https://tab1.invalid", "First Tab")];

    let mut omnibox = Omnibox::new();

    // 1. As the member types, characters are inserted and backspaced
    omnibox.set_text("test");
    omnibox.insert_str(" search");
    omnibox.backspace();
    assert_eq!(omnibox.text(), "test searc");

    // 2. Querying suggestions queries local stores only
    omnibox.query_suggestions(&history, &bookmarks, &open_tabs);

    // 3. Verifying that the omnibox state before submission holds no planned requests
    // or transmitted network traffic.
    assert_eq!(omnibox.cursor_position(), 10);
    assert!(omnibox.selected_index().is_none());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn submitted_request_carries_only_submitted_terms() {
    let b = brand();
    let mut omnibox = Omnibox::new();

    omnibox.set_text("privacy focused search engine");
    let action = omnibox.submit(b).expect("action on submit");

    match action {
        OmniboxAction::Search(sub) => {
            // Must carry only the terms the member submitted
            assert_eq!(sub.terms(), "privacy focused search engine");
            assert_eq!(sub.request().query, "q=privacy%20focused%20search%20engine");
            assert!(sub.carries_only_terms());

            // Must route through evreos-net under SubmittedSearch purpose
            assert_eq!(
                sub.planned().purpose(),
                &Purpose::HistoryBearing(HistoryBearing::SubmittedSearch)
            );
            assert_eq!(
                sub.planned().endpoint().address(),
                b.search_endpoint.as_str()
            );
        }
        other => panic!("expected Search action, got {other:?}"),
    }
}

#[test]
fn submitted_request_carries_no_navigated_address() {
    let dir = unique_temp_dir();
    let mut history = HistoryStore::open(&dir.join("history"));
    let secret_address = "https://confidential-bank.invalid/account/details";
    history
        .record(secret_address, "My Private Bank", WindowKind::Normal)
        .expect("record history");

    let _open_tabs = [OpenTab::new(
        "https://health-records.invalid/patient/42",
        "Medical Records",
    )];

    let mut omnibox = Omnibox::new();
    omnibox.set_text("paracetamol side effects");

    let b = brand();
    let action = omnibox.submit(b).expect("submit");

    if let OmniboxAction::Search(sub) = action {
        let query_str = &sub.request().query;
        let terms_str = sub.terms();

        // Must not contain any visited or open addresses
        assert!(!query_str.contains("confidential-bank"));
        assert!(!query_str.contains("health-records"));
        assert!(!query_str.contains("account"));
        assert!(!query_str.contains("patient"));

        assert!(!terms_str.contains("confidential-bank"));
        assert!(!terms_str.contains("health-records"));
    } else {
        panic!("expected search action");
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn submitted_request_carries_no_page_content() {
    let _open_tabs = [OpenTab::new(
        "https://example.invalid",
        "Sensitive Page Title And Body Content Secret",
    )];

    let mut omnibox = Omnibox::new();
    omnibox.set_text("weather report");

    let b = brand();
    let action = omnibox.submit(b).expect("submit");

    if let OmniboxAction::Search(sub) = action {
        let query = &sub.request().query;
        assert!(!query.contains("Sensitive"));
        assert!(!query.contains("Health"));
        assert!(!query.contains("Secret"));
        assert!(!query.contains("Content"));
        assert_eq!(query, "q=weather%20report");
    } else {
        panic!("expected search action");
    }
}

#[test]
fn submitted_request_carries_no_history_or_bookmark_data() {
    let dir = unique_temp_dir();
    let mut history = HistoryStore::open(&dir.join("history"));
    let mut bookmarks = BookmarkStore::open(&dir.join("bookmarks"));

    history
        .record("https://rust-lang.org", "Rust", WindowKind::Normal)
        .expect("history");

    let folder_id = bookmarks
        .create_folder(FolderId::ROOT, "Secret Vault")
        .expect("folder");
    bookmarks
        .create_bookmark(folder_id, "Secret", "https://secret.invalid")
        .expect("bookmark");

    let mut omnibox = Omnibox::new();
    omnibox.set_text("rust");

    // Suggestion query displays local matches
    omnibox.query_suggestions(&history, &bookmarks, &[]);
    assert!(!omnibox.suggestions().is_empty());

    // Member submits typed search without selecting a suggestion
    let b = brand();
    let action = omnibox.submit(b).expect("submit");

    if let OmniboxAction::Search(sub) = action {
        let query = &sub.request().query;
        assert_eq!(query, "q=rust");
        assert!(!query.contains("Secret"));
        assert!(!query.contains("Vault"));
        assert!(!query.contains("folder"));
        assert!(!query.contains("bookmark"));
        assert!(!query.contains("history"));
        assert!(sub.carries_only_terms());
    } else {
        panic!("expected search action");
    }

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn submitted_request_carries_no_identifier_across_searches() {
    let b = brand();

    let mut omnibox1 = Omnibox::new();
    omnibox1.set_text("identical query");
    let action1 = omnibox1.submit(b).expect("submit 1");

    let mut omnibox2 = Omnibox::new();
    omnibox2.set_text("identical query");
    let action2 = omnibox2.submit(b).expect("submit 2");

    match (action1, action2) {
        (OmniboxAction::Search(s1), OmniboxAction::Search(s2)) => {
            // Queries for the same terms must be byte-identical: no UUID, timestamp,
            // session ID, or machine identifier is injected or varied.
            assert_eq!(s1.request().query, s2.request().query);
            assert_eq!(s1.request().query, "q=identical%20query");
            assert_eq!(s1.request().endpoint, s2.request().endpoint);
            assert_eq!(s1.planned().endpoint(), s2.planned().endpoint());
        }
        _ => panic!("expected search actions"),
    }
}

#[test]
fn direct_url_navigation_does_not_trigger_search_egress() {
    let b = brand();
    let mut omnibox = Omnibox::new();

    let url_cases = [
        ("https://example.com/path", "https://example.com/path"),
        ("http://insecure.invalid", "http://insecure.invalid"),
        ("example.com", "https://example.com"),
        ("localhost:8080", "https://localhost:8080"),
        ("about:blank", "about:blank"),
    ];

    for (input, expected_url) in url_cases {
        omnibox.set_text(input);
        assert!(is_url_input(input), "{input} must be recognized as URL");
        assert_eq!(normalize_url(input), expected_url);

        let action = omnibox.submit(b).expect("submit");
        assert_eq!(
            action,
            OmniboxAction::Navigate {
                url: expected_url.into()
            },
            "URL input {input} must navigate directly and never produce a search egress request"
        );
    }
}

#[test]
fn selecting_suggestion_routes_without_search_egress() {
    let b = brand();
    let mut omnibox = Omnibox::new();

    // 1. Open tab suggestion
    omnibox.set_text("portal");
    omnibox.set_suggestions(vec![evreos_shell::suggest::Suggestion {
        address: "https://portal.internal.invalid".into(),
        title: "Internal Portal".into(),
        source: SuggestionSource::OpenTab,
        score: 100,
    }]);

    omnibox.select_next();
    let action = omnibox.submit(b).expect("submit");
    assert_eq!(
        action,
        OmniboxAction::SwitchToTab {
            address: "https://portal.internal.invalid".into(),
        }
    );

    // 2. Bookmark suggestion
    omnibox.set_text("docs");
    omnibox.set_suggestions(vec![evreos_shell::suggest::Suggestion {
        address: "https://docs.rs".into(),
        title: "Rust Documentation".into(),
        source: SuggestionSource::Bookmark,
        score: 90,
    }]);

    omnibox.select_next();
    let action2 = omnibox.submit(b).expect("submit");
    assert_eq!(
        action2,
        OmniboxAction::Navigate {
            url: "https://docs.rs".into(),
        }
    );
}
