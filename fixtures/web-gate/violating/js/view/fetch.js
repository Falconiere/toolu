export async function load() {
  return fetch("/api/v1/status");
}
