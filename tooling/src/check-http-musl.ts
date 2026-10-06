/** Release musl TLS, static linkage, size, and dependency check for #417. */
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { delimiter, join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../..");
const TARGET = "x86_64-unknown-linux-musl";
const BUDGET_BYTES = 4 * 1024 * 1024;
const targetDir = resolve(ROOT, process.env.CARGO_TARGET_DIR ?? "target");
const environment = {
  ...process.env,
  PATH: [join(homedir(), ".cargo", "bin"), process.env.PATH ?? ""].join(delimiter),
};

function run(command: string, args: string[], capture = false): string {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    env: environment,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} ${args.join(" ")} exited ${result.status ?? "without status"}`);
  }
  return result.stdout;
}

function checkStaticAndSize(path: string): number {
  const description = run("file", [path], true);
  if (!/(static-pie|statically) linked/.test(description)) {
    throw new Error(`${path} is not statically linked: ${description.trim()}`);
  }
  const bytes = statSync(path).size;
  if (bytes > BUDGET_BYTES) {
    throw new Error(`${path} is ${bytes} bytes, over ${BUDGET_BYTES}`);
  }
  return bytes;
}

function probeManifest(): string {
  return `[package]
name = "toolu-http-size-probe"
version = "0.0.0"
edition = "2024"

[dependencies]
toolu-http = { path = ${JSON.stringify(join(ROOT, "crates/core/http"))} }
toolu-runtime = { path = ${JSON.stringify(join(ROOT, "crates/core/runtime"))} }

[profile.release]
lto = "fat"
codegen-units = 1
strip = true
panic = "unwind"
`;
}

const PROBE_SOURCE = `use toolu_http::{Auth, Client, Config};
use toolu_runtime::env::Env;

fn main() {
  let url = std::env::args().nth(1).unwrap_or_else(|| "https://api.example.test/".into());
  let client = Client::new(Config::default(), &Env::process()).expect("client");
  let body = client.get_bytes(&url, &Auth::None).expect("request");
  println!("{}", body.len());
}
`;

function main(): void {
  run("cargo", [
    "test",
    "--release",
    "--target",
    TARGET,
    "-p",
    "toolu-http",
    "--test",
    "transport",
  ]);
  run("cargo", ["build", "--release", "--target", TARGET, "-p", "toolu-cli"]);
  const cli = join(targetDir, TARGET, "release", "toolu");
  const cliBytes = checkStaticAndSize(cli);

  const temporary = mkdtempSync(join(tmpdir(), "toolu-http-size-"));
  try {
    mkdirSync(join(temporary, "src"));
    writeFileSync(join(temporary, "Cargo.toml"), probeManifest());
    writeFileSync(join(temporary, "src/main.rs"), PROBE_SOURCE);
    const probeTarget = join(targetDir, "http-probe");
    run("cargo", [
      "build",
      "--release",
      "--target",
      TARGET,
      "--manifest-path",
      join(temporary, "Cargo.toml"),
      "--target-dir",
      probeTarget,
    ]);
    const probe = join(probeTarget, TARGET, "release", "toolu-http-size-probe");
    const probeBytes = checkStaticAndSize(probe);
    const tree = run(
      "cargo",
      ["tree", "-p", "toolu-http", "-e", "normal", "--prefix", "none"],
      true,
    );
    if (/^(tokio|hyper)(?: |$)/im.test(tree)) {
      throw new Error("unexpected async HTTP dependency");
    }
    process.stdout.write(
      `static musl TLS tests passed; toolu=${cliBytes} bytes, linked HTTP probe=${probeBytes} bytes (budget=${BUDGET_BYTES})\n`,
    );
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

if (existsSync(join(ROOT, "Cargo.toml"))) {
  main();
} else {
  throw new Error(`not a Cargo workspace: ${ROOT}`);
}
