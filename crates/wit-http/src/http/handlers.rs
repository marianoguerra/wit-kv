//! Internal axum handler functions that bridge HTTP to WitResource.

use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};

use crate::content::format::{decode_request, encode_list_response, encode_response};
use crate::core::query::CollectionQuery;
use crate::core::resource::WitResource;
use crate::http::error::ApiError;
use crate::http::extractors::{AcceptFormat, RequestFormat};
use crate::http::response::FormatResponse;

pub(crate) async fn handle_get<R: WitResource>(
    State(resource): State<Arc<R>>,
    Path(id): Path<String>,
    AcceptFormat(fmt): AcceptFormat,
) -> Result<Response, ApiError> {
    let item = resource.get(&id).await?;
    let bytes = encode_response::<R::Item>(fmt, &item)?;
    Ok(FormatResponse::new(fmt, bytes, StatusCode::OK).into_response())
}

pub(crate) async fn handle_set<R: WitResource>(
    State(resource): State<Arc<R>>,
    Path(id): Path<String>,
    RequestFormat(req_fmt): RequestFormat,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let item = decode_request::<R::Item>(req_fmt, &body)?;
    resource.set(&id, item).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn handle_delete<R: WitResource>(
    State(resource): State<Arc<R>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    resource.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn handle_list<R: WitResource>(
    State(resource): State<Arc<R>>,
    Query(query): Query<CollectionQuery>,
    AcceptFormat(fmt): AcceptFormat,
) -> Result<Response, ApiError> {
    let items = resource.list(query).await?;
    let bytes = encode_list_response::<R::Item>(fmt, &items)?;
    Ok(FormatResponse::new(fmt, bytes, StatusCode::OK).into_response())
}
