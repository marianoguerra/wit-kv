//! Collection query parameters for pagination and filtering.

/// Query parameters for collection (list) endpoints.
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct CollectionQuery {
    pub start: Option<String>,
    pub end: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
