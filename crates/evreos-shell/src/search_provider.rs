//! Search provider setting (FR-003a, Q-E2).
//!
//! Under Q-E2, DuckDuckGo is the default search provider, held to FR-003a's boundary:
//! the submitted search carries only the terms the member submitted, nothing before
//! submission, and no identifier across searches.
//!
//! The endpoint is resolved from the brand configuration in [`crate::brand`] as an
//! [`evreos_net::Endpoint`] the egress crate will accept and never from a literal.
//!
//! The provider is changeable by the member from first run without penalty.
//! Changing the provider — by the member or by brand configuration — changes only
//! which service receives the query and never what the query carries.
//!
//! No paid-placement or revenue-sharing arrangement exists in v1, so no disclosure
//! surface is present.

#![forbid(unsafe_code)]

use crate::brand::{Brand, SearchRequest, percent_encode};

/// Search provider configuration.
///
/// Holds the provider name and the search endpoint resolved from brand configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchProviderSetting {
    /// Human-readable provider identifier (default is "DuckDuckGo" per Q-E2).
    pub provider: String,
    /// Search endpoint URL template.
    pub endpoint: String,
}

impl SearchProviderSetting {
    /// Create a search provider setting with provider name and endpoint.
    pub fn new(provider: impl Into<String>, endpoint: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            endpoint: endpoint.into(),
        }
    }

    /// Default privacy-preserving provider resolved against brand configuration.
    ///
    /// Names DuckDuckGo per Q-E2, with its endpoint drawn from the embedded brand.
    pub fn default_provider() -> Self {
        Self::from_brand(crate::brand::brand())
    }

    /// Construct the default search provider setting for a specific brand configuration.
    pub fn from_brand(brand: &Brand) -> Self {
        Self {
            provider: "DuckDuckGo".to_string(),
            endpoint: brand.search_endpoint.clone(),
        }
    }

    /// The human-readable provider name.
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// The configured endpoint URL.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Change the provider setting (by the member, from first run without penalty).
    pub fn change_provider(&mut self, provider: impl Into<String>, endpoint: impl Into<String>) {
        self.provider = provider.into();
        self.endpoint = endpoint.into();
    }

    /// Resolve the egress [`evreos_net::Endpoint`] from brand configuration.
    ///
    /// Never resolves an endpoint from a string literal outside the brand seam.
    pub fn resolved_endpoint(&self, brand: &Brand) -> evreos_net::Endpoint {
        brand.search_endpoint()
    }

    /// Compose a [`SearchRequest`] for `terms`.
    ///
    /// FR-003a invariant: changing the provider changes only which service receives
    /// the query and never what the query carries.
    pub fn search_request(&self, terms: &str) -> SearchRequest {
        SearchRequest {
            endpoint: self.endpoint.clone(),
            query: format!("q={}", percent_encode(terms)),
        }
    }

    /// Plan a search request through `evreos-net` against the brand's verified endpoint.
    pub fn planned_search_request(
        &self,
        brand: &Brand,
        terms: &str,
    ) -> (evreos_net::PlannedRequest, SearchRequest) {
        let endpoint = self.resolved_endpoint(brand);
        let planned = evreos_net::request(
            evreos_net::Purpose::HistoryBearing(evreos_net::HistoryBearing::SubmittedSearch),
            endpoint,
        );
        let request = self.search_request(terms);
        (planned, request)
    }

    /// Whether any paid-placement or revenue-sharing disclosure surface is present.
    ///
    /// Unconditionally returns `false`: no such arrangement exists in v1 (FR-003a, Q-E2).
    pub fn has_paid_placement_disclosure(&self) -> bool {
        false
    }
}

impl Default for SearchProviderSetting {
    fn default() -> Self {
        Self::default_provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_provider_is_duckduckgo_per_qe2() {
        let setting = SearchProviderSetting::default();
        assert_eq!(setting.provider(), "DuckDuckGo");
        assert_eq!(
            setting.endpoint(),
            crate::brand::brand().search_endpoint.as_str()
        );
    }

    #[test]
    fn changing_provider_preserves_query_format() {
        let mut setting = SearchProviderSetting::default();
        let req1 = setting.search_request("test query");
        assert_eq!(req1.query, "q=test%20query");

        setting.change_provider("CustomSearch", "https://custom.invalid/search");
        let req2 = setting.search_request("test query");
        assert_eq!(req2.endpoint, "https://custom.invalid/search");
        assert_eq!(req2.query, "q=test%20query");
        assert_eq!(req1.query, req2.query);
    }

    #[test]
    fn no_paid_placement_disclosure_surface() {
        let setting = SearchProviderSetting::default();
        assert!(!setting.has_paid_placement_disclosure());
    }
}
