import { peerDashboardUrl } from "../logic/status.js";
import { $ } from "./dom.js";

// The `[ OPEN <NAME> DASHBOARD ↗ ]` link, shown only for a safe peer root URL.
export function showPeerLink(link, machine) {
  const url = peerDashboardUrl(machine?.dashboard_url);
  link.hidden = !url;
  if (!url) return;
  link.href = url;
  link.textContent = `[ OPEN ${machine.name.toUpperCase()} DASHBOARD ↗ ]`;
}

export function renderPeer(machine) {
  const banner = $("peer-banner");
  banner.hidden = !machine;
  $("peer-name").textContent = machine?.name.toUpperCase() || "";
  $("hosts-help").textContent = machine
    ? "Read-only on a peer."
    : "Pause stops new launches; running workers continue. Resume also clears a cooldown.";
  showPeerLink($("peer-link"), machine);
}
