import { peerDashboardUrl } from "../logic/status.js";

const $ = (id) => document.getElementById(id);

export function renderPeer(machine) {
  const banner = $("peer-banner");
  banner.hidden = !machine;
  $("peer-name").textContent = machine?.name.toUpperCase() || "";
  $("hosts-help").textContent = machine
    ? "Read-only on a peer."
    : "Pause stops new launches; running workers continue. Resume also clears a cooldown.";
  const link = $("peer-link");
  link.hidden = true;
  const url = peerDashboardUrl(machine?.dashboard_url);
  if (!url) return;
  link.href = url;
  link.textContent = `[ OPEN ${machine.name.toUpperCase()} DASHBOARD ↗ ]`;
  link.hidden = false;
}
