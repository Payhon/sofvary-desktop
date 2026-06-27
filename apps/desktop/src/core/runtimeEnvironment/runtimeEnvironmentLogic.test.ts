import test from "node:test";
import assert from "node:assert/strict";
import type { RuntimeEnvironmentStatus } from "../../types";
import {
  canActivateRuntimeEnvironmentVersion,
  canInstallRuntimeEnvironment,
  formatRuntimeEnvironmentStatus,
  getDefaultRuntimeEnvironmentVersion,
  getRuntimeEnvironmentRequirementIssue,
  getRuntimeEnvironmentActionLabel,
  runtimeRequiresNodeToolchain,
  runtimeEnvironmentInstallKey,
  sortRuntimeEnvironmentVersions,
} from "./runtimeEnvironmentLogic";

const baseStatus: RuntimeEnvironmentStatus = {
  catalog: {
    kind: "nodejs",
    label: "Node.js Toolchain",
    description: "Managed Node.js plus pnpm",
    requiredTools: ["node", "pnpm"],
    supported: true,
    versions: [
      version("22.22.3", false),
      version("24.16.0", true),
    ],
  },
  activeVersion: null,
  installState: "not-installed",
  detail: "Missing",
  source: "missing",
  supported: true,
  node: null,
  pnpm: null,
  lastInstall: null,
};

test("sortRuntimeEnvironmentVersions keeps recommended supported version first", () => {
  assert.deepEqual(
    sortRuntimeEnvironmentVersions(baseStatus.catalog.versions).map((item) => item.version),
    ["24.16.0", "22.22.3"],
  );
  assert.equal(getDefaultRuntimeEnvironmentVersion(baseStatus)?.version, "24.16.0");
});

test("runtime environment action rules distinguish install, active, and switch", () => {
  const selected = getDefaultRuntimeEnvironmentVersion(baseStatus);
  assert.equal(canInstallRuntimeEnvironment(baseStatus, selected, null), true);
  assert.equal(canActivateRuntimeEnvironmentVersion(baseStatus, selected, null), false);
  assert.equal(getRuntimeEnvironmentActionLabel(baseStatus, selected, null), "Install");

  const installed: RuntimeEnvironmentStatus = {
    ...baseStatus,
    activeVersion: "24.16.0",
    installState: "installed",
    source: "managed",
  };
  assert.equal(getRuntimeEnvironmentActionLabel(installed, selected, null), "Active");

  const older = installed.catalog.versions.find((item) => item.version === "22.22.3") ?? null;
  assert.equal(canActivateRuntimeEnvironmentVersion(installed, older, null), true);
  assert.equal(getRuntimeEnvironmentActionLabel(installed, older, null), "Switch");
});

test("runtime environment install key and status text are stable", () => {
  const selected = getDefaultRuntimeEnvironmentVersion(baseStatus);
  assert(selected);
  assert.equal(runtimeEnvironmentInstallKey(baseStatus, selected), "nodejs:24.16.0");

  const installed: RuntimeEnvironmentStatus = {
    ...baseStatus,
    installState: "installed",
    source: "managed",
    node: {
      name: "node",
      ok: true,
      version: "v24.16.0",
      executable: "node.exe",
      source: "managed",
      detail: "Detected",
    },
    pnpm: {
      name: "pnpm",
      ok: true,
      version: "10.12.3",
      executable: "pnpm.cmd",
      source: "managed",
      detail: "Detected",
    },
  };

  assert.equal(formatRuntimeEnvironmentStatus(installed), "Sofvary managed / Node v24.16.0 / pnpm 10.12.3");
});

test("runtime requirement check blocks catalog-declared node toolchains until node and pnpm are ready", () => {
  const issue = getRuntimeEnvironmentRequirementIssue("react-sqlite", [baseStatus], ["nodejs"]);

  assert.equal(runtimeRequiresNodeToolchain("react-sqlite", ["nodejs"]), true);
  assert.equal(runtimeRequiresNodeToolchain("custom-runtime", ["nodejs"]), true);
  assert.equal(runtimeRequiresNodeToolchain("react-sqlite"), false);
  assert.equal(runtimeRequiresNodeToolchain("static-html", []), false);
  assert.match(issue?.message ?? "", /requires the Sofvary-managed Node\.js Toolchain/);
  assert.match(issue?.message ?? "", /before previewing/);
  assert.match(issue?.message ?? "", /Node\.js/);
  assert.match(issue?.message ?? "", /pnpm/);
});

test("runtime requirement check passes installed node toolchain", () => {
  const installed: RuntimeEnvironmentStatus = {
    ...baseStatus,
    installState: "installed",
    source: "managed",
    node: {
      name: "node",
      ok: true,
      version: "v24.16.0",
      executable: "node.exe",
      source: "managed",
      detail: "Detected",
    },
    pnpm: {
      name: "pnpm",
      ok: true,
      version: "10.12.3",
      executable: "pnpm.cmd",
      source: "managed",
      detail: "Detected",
    },
  };

  assert.equal(getRuntimeEnvironmentRequirementIssue("react-vite", [installed], ["nodejs"]), null);
});

test("runtime requirement check rejects external PATH tools for managed runtimes", () => {
  const installed: RuntimeEnvironmentStatus = {
    ...baseStatus,
    installState: "installed",
    source: "external-path",
    node: {
      name: "node",
      ok: true,
      version: "v24.16.0",
      executable: "node",
      source: "external-path",
      detail: "Detected",
    },
    pnpm: {
      name: "pnpm",
      ok: true,
      version: "10.12.3",
      executable: "pnpm",
      source: "external-path",
      detail: "Detected",
    },
  };

  const issue = getRuntimeEnvironmentRequirementIssue("react-sqlite", [installed], ["nodejs"]);

  assert.match(issue?.message ?? "", /managed Node\.js sidecars/);
  assert.match(issue?.message ?? "", /external PATH tools are not enough/);
});

test("active install blocks install and activation actions", () => {
  const selected = getDefaultRuntimeEnvironmentVersion(baseStatus);
  assert.equal(canInstallRuntimeEnvironment(baseStatus, selected, "nodejs:24.16.0"), false);
  assert.equal(canActivateRuntimeEnvironmentVersion(baseStatus, selected, "nodejs:24.16.0"), false);
  assert.equal(getRuntimeEnvironmentActionLabel(baseStatus, selected, "nodejs:24.16.0"), "Installing");
});

function version(version: string, recommended: boolean) {
  return {
    version,
    label: `Node.js ${version}`,
    channel: recommended ? "LTS" : "Maintenance LTS",
    recommended,
    supported: true,
    platform: "windows-x64",
    artifactUrl: `https://nodejs.org/dist/v${version}/node-v${version}-win-x64.zip`,
    sha256: "abc123",
    pnpmVersion: "10.12.3",
    pnpmArtifactUrl: "https://registry.npmjs.org/pnpm/-/pnpm-10.12.3.tgz",
    pnpmIntegrity: "sha512-test",
  };
}
