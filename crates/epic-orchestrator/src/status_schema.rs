//! The status API surface used to generate its checked reference.

use std::fmt::Write as _;

struct Endpoint {
  path: &'static str,
  fields: &'static str,
  purpose: &'static str,
}

const ENDPOINTS: &[Endpoint] = &[
  Endpoint {
    path: "/healthz",
    fields: "version, uptime_ms, journal_seq",
    purpose: "Engine health",
  },
  Endpoint {
    path: "/api/v1/overview",
    fields: "engine, paused, journal_seq, epics, issues, attention, counts, attention_count, hosts, machine, jobs",
    purpose: "Current aggregate",
  },
  Endpoint {
    path: "/api/v1/epics",
    fields: "items (key, paused, counts, attention_count)",
    purpose: "Registered epics",
  },
  Endpoint {
    path: "/api/v1/epics/{id}",
    fields: "key, paused, graph, issues, queue",
    purpose: "One epic board",
  },
  Endpoint {
    path: "/api/v1/issues/{key}",
    fields: "key, epic, stage, phase, phase_age_ms, host, model, effort, pr, last_gate_verdict, queue_position, launches, phase_history, recent_events",
    purpose: "One worker",
  },
  Endpoint {
    path: "/api/v1/agents",
    fields: "items (state, key, epic, worktree, session_id)",
    purpose: "Worker sessions",
  },
  Endpoint {
    path: "/api/v1/hosts",
    fields: "items (host, cap, running, cooldown_until, reason)",
    purpose: "Host capacity",
  },
  Endpoint {
    path: "/api/v1/machine",
    fields: "load, available_memory_bytes, steal, pressure, policy, agent_leases, job_leases",
    purpose: "Shared resources",
  },
  Endpoint {
    path: "/api/v1/jobs",
    fields: "running, waiting",
    purpose: "Expensive jobs",
  },
  Endpoint {
    path: "/api/v1/queue",
    fields: "repositories (repo, items)",
    purpose: "Ready work by repository",
  },
  Endpoint {
    path: "/api/v1/attention",
    fields: "items (kind, key, epic, note, seq, state, age_ms)",
    purpose: "Live attention",
  },
  Endpoint {
    path: "/api/v1/metrics",
    fields: "epic, since, generated_at, records, issues, lead_time_ms, phases, ready_to_merged_ms, rebase_rounds, prompts, judgments, wakeups, github, jobs, launches, engine",
    purpose: "Epic run metrics",
  },
  Endpoint {
    path: "/api/v1/fleet",
    fields: "summary (machines, reachable, hosts, in_flight, attention_count), machines (name, local, state, reason, age_ms, last_seen, engine, paused, journal_seq, epics, in_flight, hosts, counts, attention_count)",
    purpose: "This machine and every peer",
  },
  Endpoint {
    path: "/api/v1/fleet/peers/{name}/{route}",
    fields: "the peer's document for route overview, epics, epics/{id}, issues/{key}, agents, hosts, machine, jobs, queue or attention",
    purpose: "One peer's document, read through this server",
  },
  Endpoint {
    path: "/api/v1/events",
    fields: "events, oldest_seq, next_seq",
    purpose: "Journal replay",
  },
  Endpoint {
    path: "/api/v1/events/stream",
    fields: "SSE id, event, data",
    purpose: "Live journal feed",
  },
];

/// Render the versioned API reference from the endpoint definitions.
pub(crate) fn render() -> String {
  let mut out = String::from(
    "# Epic status API\n\nGenerated from `crates/epic-orchestrator/src/status_schema.rs`. Regenerate with `TOOLU_UPDATE_DOCS=1 cargo test -p toolu-epic-orchestrator status_api_doc`.\n\nThe resident engine serves HTTP/1.1 on `127.0.0.1:7717` by default. `toolu epic serve --http ADDR` selects another address; `--no-http` disables the listener. Non-loopback binds require a status token from `toolu/secrets.json` or `TOOLU_EPIC_STATUS_TOKEN` / `TOOLU_EPIC_TOKEN`, sent as `Authorization: Bearer <token>`.\n\nOnly GET and HEAD are accepted. HEAD has no body. Every JSON body contains `schema: 1`; every response has `Cache-Control: no-store` and no CORS header. Errors use fixed JSON codes. The listener accepts only its bound Host and loopback aliases.\n\n| Path | Purpose | Fields after `schema` |\n|---|---|---|\n",
  );
  for endpoint in ENDPOINTS {
    let _written = writeln!(
      out,
      "| `{}` | {} | {} |",
      endpoint.path, endpoint.purpose, endpoint.fields
    );
  }
  out.push_str("\n`/api/v1/events` accepts `since` (default 0) and `limit` (default 100, maximum 500). An event contains `seq`, `at`, `kind`, `name`, `epic`, `key`, and a bounded `note`. The stream emits `id: <seq>`, `event: journal`, and `data: <JSON>`; `Last-Event-ID` overrides `since`. A cursor older than retained history receives a `reset` event.\n\n`/api/v1/metrics` accepts `epic` and `since` (`YYYY-MM-DD` or an ISO-8601 UTC time) and serves the document of `toolu epic metrics --json` (`$defs/epicMetrics` of `toolu commands --schema`), recomputed at most once a minute per query.\n\n## Fleet over a private network

An engine can show other machines' engines beside its own. List each peer under `epic.peers` in `toolu.config.json` with a `name` and a `url`, and put its bearer token in `peer_tokens` of `toolu/secrets.json` (mode 0600) or in `TOOLU_EPIC_PEER_TOKENS`, a JSON object of peer name to token. A token never goes on the command line. The engine reads its peers at startup; `toolu epic status --fleet` reads them on every run.

A peer accepts only a `Host` equal to its bound address, so the URL names the peer's bound IP and port, for example `http://192.168.1.20:7717` for an engine started with `--http 0.0.0.0:7717` or `epic.http.bind` `0.0.0.0`. A DNS name, or a URL without the port, gets `403` (state `unauthorized`, reason `host`). With TLS in front of a peer, the proxy must forward to the engine with `Host` set to the engine's address: nginx's `proxy_pass` does this by default, and Caddy needs `header_up Host {upstream_hostport}`.

A token sent to an `http://` peer crosses the network in clear text. Use plain HTTP only on a private network. A status server must not be exposed to the public internet without its token and TLS in front of it.

`/api/v1/fleet` lists this machine first (`name` `local`, `local: true`), then each peer in config order. Peers are fetched in parallel with a 2 s deadline, at most once a second for requests and every 30 s in the background while peers are configured. A peer's `state` is `ok`, `unreachable` (no reply, timeout, redirect, a body over 1 MiB or a server error), `unauthorized` (`reason` `token` for `401`, `host` for the engine's `403 foreign_host`, otherwise `null`) or `version-mismatch` (a status API other than schema 1). `last_seen` is the peer's last `ok` reply since this engine started, and `age_ms` its age; only `ok` machines carry data and count in `summary`. In-flight issues are those whose stage is `running` or `awaiting_merge`. Peer fields are allow-listed, strings are cut to 160 characters, and every configured secret is redacted. Peer requests are read-only `GET`s; a peer is never asked to act.

`/api/v1/fleet/peers/{name}/{route}` returns a peer's own document, redacted, so the browser needs only this server's address. `{name}` is percent-decoded and matches a configured peer exactly; `{id}` and `{key}` match `[A-Za-z0-9._-]{1,128}`. An unknown peer or route is `404`; a failing peer is `502` (`peer_unreachable`, `peer_unauthorized`, `peer_version_mismatch`), and more than eight proxied calls at once get `503 proxy_limit`.

The dashboard at `/` uses these same-origin endpoints. With peers configured it shows a fleet summary and a machine switcher; a peer's panels are read through the proxy every 5 s. Waiting jobs are recent refused admissions; their markers expire after two minutes. Queue entries are the engine's merge queue: per repository, ready issues with a pull request, ordered by `unblocks`, then ready time, then key; `head` marks the one the engine works.\n");
  out
}

#[cfg(test)]
#[path = "tests/status_schema_test.rs"]
mod tests;
