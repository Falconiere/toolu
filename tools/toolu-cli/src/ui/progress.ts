import type { InstallProgress } from "../plugins/install";

/** A single terminal line; kept separate from the durable install report. */
export function installProgress(
  host: string,
  write: (text: string) => void,
  columns: () => number = () => process.stdout.columns || 80,
): { update: (progress: InstallProgress) => void; stop: () => void } {
  let current: InstallProgress = { completed: 0, total: 0, label: "Preparing" };
  let frame = 0;
  const render = (): void => {
    const { completed, total, label } = current;
    const ratio = total === 0 ? 0 : completed / total;
    const filled = Math.floor(ratio * 20);
    const bar = `${"━".repeat(filled)}${"─".repeat(20 - filled)}`;
    const spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"][frame++ % 10];
    const line = `  ${spinner} ${host} [${bar}] ${completed}/${total} ${Math.floor(ratio * 100)}% · ${label}`;
    write(`\r\u001b[2K${line.slice(0, Math.max(0, columns() - 1))}`);
  };
  const timer = setInterval(render, 80);
  timer.unref();
  return {
    update: (progress) => {
      current = progress;
      render();
    },
    stop: () => {
      clearInterval(timer);
      write("\r\u001b[2K");
    },
  };
}
