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
  out.push_str("\n`/api/v1/events` accepts `since` (default 0) and `limit` (default 100, maximum 500). An event contains `seq`, `at`, `kind`, `name`, `epic`, `key`, and a bounded `note`. The stream emits `id: <seq>`, `event: journal`, and `data: <JSON>`; `Last-Event-ID` overrides `since`. A cursor older than retained history receives a `reset` event.\n\nThe dashboard at `/` uses these same-origin endpoints. Waiting jobs are recent refused admissions; their markers expire after two minutes. Queue entries are ready worker issues from registered epic graphs.\n");
  out
}

#[cfg(test)]
#[path = "tests/status_schema_test.rs"]
mod tests;
