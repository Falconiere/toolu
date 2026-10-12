//! Route each sub-issue to a worker host, model and effort (`scripts/route.ts`).
//!
//! Jev scores each issue's implementation complexity into a tier; the routing
//! table maps tier to model and effort per host; hosts fill by free capacity,
//! skipping any host cooling down after a usage limit. Routes persist in
//! `<state>/routes/<key>.json`, so relaunches keep their host and model.

mod assign;
mod capacity;
mod pool;
mod render;
mod score;
mod table;
mod tier;

pub(crate) use assign::{Route, RouteOptions, route_issues};
pub(crate) use pool::{HostPool, LaunchRoute, load_host_pool, load_launch_route, parse_hosts};
pub(crate) use render::render_routes;
pub(crate) use score::jev_from_env;
pub(crate) use table::{RoutingTable, load_table};
