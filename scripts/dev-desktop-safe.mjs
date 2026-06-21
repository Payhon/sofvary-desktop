import { spawn } from "node:child_process";

const pnpmCli = process.env.npm_execpath;

if (!pnpmCli) {
  console.error("Unable to locate the active package manager executable.");
  process.exit(1);
}

const child = spawn(
  process.execPath,
  [pnpmCli, "--filter", "@sofvary/desktop", "tauri", "dev"],
  {
    env: {
      ...process.env,
      SOFVARY_SAFE_SHELL: "1",
    },
    stdio: "inherit",
  },
);

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }

  process.exit(code ?? 1);
});
