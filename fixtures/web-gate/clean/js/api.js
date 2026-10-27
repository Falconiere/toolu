export async function loadStatus() {
  const response = await fetch("/api/v1/status");
  return response.json();
}
