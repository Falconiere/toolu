//! The listener's read routes: the dashboard, JSON documents, the journal
//! page and the live event stream.

use std::net::TcpStream;
use std::sync::atomic::Ordering;

use toolu_runtime::config::secrets::Secrets;

use crate::dashboard::{PAGE, asset};
use crate::http::{Context, STREAMS};
use crate::http_access::authorized;
use crate::http_request::Request;
use crate::http_response::{READ_METHODS, Response, json_response, send, stream_headers};
use crate::http_stream;
use crate::status_api;

/// Build a finite response from embedded assets, local status, or a peer.
pub(crate) fn finite_response(
  context: &Context,
  path: &str,
  query: &str,
  request: &Request,
  secrets: &Secrets,
) -> Response {
  if path == "/" {
    return Response {
      status: 200,
      content_type: "text/html; charset=utf-8",
      body: PAGE.to_vec(),
      head: request.method == "HEAD",
      close: false,
      allow: READ_METHODS,
      cache: "no-store",
    };
  }
  if let Some(asset) = asset(path) {
    return Response {
      status: 200,
      content_type: asset.content_type,
      body: asset.bytes.to_vec(),
      head: request.method == "HEAD",
      close: false,
      allow: READ_METHODS,
      cache: "public, max-age=31536000, immutable",
    };
  }
  if let Some(response) = engine_route(context, path, query, request, secrets) {
    return response;
  }
  let fleet = &context.fleet;
  if let Some(response) =
    crate::http_fleet::response(fleet, &context.world, &context.paths, secrets, path)
  {
    return response;
  }
  let world = match context.world.lock() {
    Ok(world) => world.clone(),
    Err(_err) => return Response::error(500, "status_unavailable"),
  };
  let document = status_api::route(
    &world,
    &context.paths,
    path,
    u64::try_from(context.started.elapsed().as_millis()).unwrap_or(u64::MAX),
    secrets,
  );
  json_response(document)
}

/// Routes served from this engine's own caches and journal.
fn engine_route(
  context: &Context,
  path: &str,
  query: &str,
  request: &Request,
  secrets: &Secrets,
) -> Option<Response> {
  if let Some(response) = crate::http_logs::response(context, path, query, request, secrets) {
    return Some(response);
  }
  if let Some(key) = path
    .strip_prefix("/api/v1/issues/")
    .and_then(|rest| rest.strip_suffix("/screen"))
  {
    if secrets.status_token().is_none() {
      return Some(Response::error(403, "control_disabled"));
    }
    if !authorized(secrets, request.authorization.as_deref()) {
      return Some(Response::error(401, "unauthorized"));
    }
    return Some(
      context
        .screen
        .response(key, &context.herdr_calls, &context.world, secrets),
    );
  }
  Some(match path {
    "/api/v1/agents/live" => context
      .live
      .response(&context.herdr_calls, &context.world, secrets),
    "/api/v1/overview" => {
      let Ok(world) = context.world.lock() else {
        return Some(Response::error(500, "status_unavailable"));
      };
      context.overview.response(&world, &context.paths, secrets)
    }
    "/api/v1/events" => http_stream::page(
      &context.paths,
      query,
      request.last_event_id.as_deref(),
      secrets,
    ),
    "/api/v1/metrics" => context.metrics.response(&context.paths, query, secrets),
    _ => return None,
  })
}

/// Serve an authorized journal stream until it closes or its token expires.
pub(crate) fn serve_events(
  stream: &mut TcpStream,
  context: &Context,
  request: &Request,
  query: &str,
  secrets: &Secrets,
) -> bool {
  if context.streams.fetch_add(1, Ordering::AcqRel) >= STREAMS {
    context.streams.fetch_sub(1, Ordering::AcqRel);
    let _sent = send(stream, &Response::error(429, "stream_limit"));
    return true;
  }
  let cursor = http_stream::cursor(query, request.last_event_id.as_deref());
  match cursor {
    Ok(cursor) if request.method == "GET" && stream_headers(stream).is_ok() => {
      http_stream::serve(
        stream,
        cursor,
        &http_stream::Feed {
          paths: &context.paths,
          secrets,
          world: &context.world,
          changed: &context.changed,
          stop: &context.stop,
          allowed: &|| {
            !context.auth_required
              || authorized(&context.secrets.current(), request.authorization.as_deref())
          },
        },
      );
    }
    Ok(_) => {}
    Err(_err) => {
      let _sent = send(stream, &Response::error(400, "bad_cursor"));
    }
  }
  context.streams.fetch_sub(1, Ordering::AcqRel);
  true
}

#[cfg(test)]
#[path = "tests/http_read_test.rs"]
mod tests;
