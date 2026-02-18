//! The central WitResource trait for CRUD operations on WIT-typed items.

use std::future::Future;

use super::error::ResourceError;
use super::query::CollectionQuery;
use super::wit_type::WitType;

/// Handler trait for a WIT-typed resource.
///
/// Implementors provide CRUD and collection operations using native
/// Rust types. The framework handles HTTP concerns (content negotiation,
/// serialization, status codes, error responses).
///
/// Each method has a default implementation returning `NotSupported`,
/// so implementors only override the operations they need.
///
/// # Example
///
/// ```ignore
/// use wit_http::{WitResource, WitType, ResourceError, CollectionQuery};
///
/// struct UserStore { /* ... */ }
///
/// impl WitResource for UserStore {
///     type Item = User;
///
///     async fn get(&self, id: &str) -> Result<User, ResourceError> {
///         // fetch user by id...
///         # todo!()
///     }
///
///     async fn set(&self, id: &str, user: User) -> Result<(), ResourceError> {
///         // store user...
///         Ok(())
///     }
/// }
/// ```
pub trait WitResource: Send + Sync + 'static {
    type Item: WitType;

    /// Get a single item by ID.
    fn get<'a>(
        &'a self,
        id: &'a str,
    ) -> impl Future<Output = Result<Self::Item, ResourceError>> + Send + 'a {
        async move {
            let _ = id;
            Err(ResourceError::NotSupported("get".to_string()))
        }
    }

    /// Create or update an item.
    fn set<'a>(
        &'a self,
        id: &'a str,
        item: Self::Item,
    ) -> impl Future<Output = Result<(), ResourceError>> + Send + 'a {
        async move {
            let _ = (id, item);
            Err(ResourceError::NotSupported("set".to_string()))
        }
    }

    /// Delete an item by ID.
    fn delete<'a>(
        &'a self,
        id: &'a str,
    ) -> impl Future<Output = Result<(), ResourceError>> + Send + 'a {
        async move {
            let _ = id;
            Err(ResourceError::NotSupported("delete".to_string()))
        }
    }

    /// List items (collection endpoint).
    fn list(
        &self,
        query: CollectionQuery,
    ) -> impl Future<Output = Result<Vec<Self::Item>, ResourceError>> + Send + '_ {
        async move {
            let _ = query;
            Err(ResourceError::NotSupported("list".to_string()))
        }
    }
}
