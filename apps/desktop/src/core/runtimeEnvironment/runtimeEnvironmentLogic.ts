import type {
  RuntimeKind,
  RuntimeEnvironmentInstallState,
  RuntimeEnvironmentSource,
  RuntimeEnvironmentStatus,
  RuntimeEnvironmentVersionOption,
} from "../../types";

const installStatePriority: Record<RuntimeEnvironmentInstallState, number> = {
  installed: 0,
  installing: -1,
  failed: 1,
  "not-installed": 2,
  unsupported: 3,
};

const nodeToolchainRuntimeKinds: RuntimeKind[] = [
  "react-vite",
  "react-sqlite",
  "ai-agent-app",
  "markdown-knowledge",
  "data-table",
  "file-processor",
  "desktop-widget",
];

export interface RuntimeEnvironmentRequirementIssue {
  runtimeKind: RuntimeKind;
  runtimeEnvironmentKind: "nodejs";
  message: string;
}

export function runtimeRequiresNodeToolchain(runtimeKind: RuntimeKind): boolean {
  return nodeToolchainRuntimeKinds.includes(runtimeKind);
}

export function getRuntimeEnvironmentRequirementIssue(
  runtimeKind: RuntimeKind,
  statuses: RuntimeEnvironmentStatus[],
): RuntimeEnvironmentRequirementIssue | null {
  if (!runtimeRequiresNodeToolchain(runtimeKind)) return null;

  const nodeStatus = statuses.find((status) => status.catalog.kind === "nodejs");
  const nodeReady = nodeStatus?.node?.ok === true;
  const pnpmReady = nodeStatus?.pnpm?.ok === true;
  const managedReady =
    nodeStatus?.source === "managed" &&
    nodeStatus.node?.source === "managed" &&
    nodeStatus.pnpm?.source === "managed";
  if (nodeStatus?.installState === "installed" && nodeReady && pnpmReady && managedReady) {
    return null;
  }

  const missingTools = [
    nodeReady ? null : "Node.js",
    pnpmReady ? null : "pnpm",
  ].filter(Boolean);
  const missingText =
    missingTools.length > 0 ? ` Missing: ${missingTools.join(", ")}.` : "";
  const sourceText =
    nodeReady && pnpmReady && !managedReady
      ? " Sofvary requires its managed Node.js sidecars for this runtime; external PATH tools are not enough."
      : "";
  const detail = nodeStatus?.detail ? ` ${nodeStatus.detail}` : "";

  return {
    runtimeKind,
    runtimeEnvironmentKind: "nodejs",
    message: `${runtimeKind} requires the Sofvary-managed Node.js Toolchain before previewing. Install it from Settings > Runtime Environment, then retry preview.${missingText}${sourceText}${detail}`,
  };
}

export function sortRuntimeEnvironmentVersions(
  versions: RuntimeEnvironmentVersionOption[],
): RuntimeEnvironmentVersionOption[] {
  return [...versions].sort((left, right) => {
    if (left.recommended !== right.recommended) return left.recommended ? -1 : 1;
    if (left.supported !== right.supported) return left.supported ? -1 : 1;
    return compareVersionDesc(left.version, right.version);
  });
}

export function getDefaultRuntimeEnvironmentVersion(
  status: RuntimeEnvironmentStatus,
): RuntimeEnvironmentVersionOption | null {
  const versions = sortRuntimeEnvironmentVersions(status.catalog.versions);
  return (
    versions.find((version) => version.version === status.activeVersion) ??
    versions.find((version) => version.recommended && version.supported) ??
    versions.find((version) => version.supported) ??
    versions[0] ??
    null
  );
}

export function canInstallRuntimeEnvironment(
  status: RuntimeEnvironmentStatus,
  version: RuntimeEnvironmentVersionOption | null,
  activeInstallKey: string | null,
): boolean {
  if (!version || !version.supported || !status.supported) return false;
  if (activeInstallKey) return false;
  return status.installState !== "installing";
}

export function canActivateRuntimeEnvironmentVersion(
  status: RuntimeEnvironmentStatus,
  version: RuntimeEnvironmentVersionOption | null,
  activeInstallKey: string | null,
): boolean {
  if (!version || !version.supported || activeInstallKey) return false;
  if (status.activeVersion === version.version) return false;
  return status.installState === "installed";
}

export function runtimeEnvironmentInstallKey(
  status: RuntimeEnvironmentStatus,
  version: RuntimeEnvironmentVersionOption,
): string {
  return `${status.catalog.kind}:${version.version}`;
}

export function formatRuntimeEnvironmentStatus(status: RuntimeEnvironmentStatus): string {
  if (status.installState === "installed") {
    const source = formatRuntimeEnvironmentSource(status.source);
    const node = status.node?.version ? `Node ${status.node.version}` : "Node ready";
    const pnpm = status.pnpm?.version ? `pnpm ${status.pnpm.version}` : "pnpm ready";
    return `${source} / ${node} / ${pnpm}`;
  }

  if (status.installState === "installing") return "Installing managed runtime environment";
  if (status.installState === "failed") {
    return status.lastInstall?.detail ?? "Runtime environment install failed";
  }
  if (status.installState === "unsupported") return "Current platform is not supported yet";
  return "Node.js Toolchain is not installed";
}

export function formatRuntimeEnvironmentSource(source: RuntimeEnvironmentSource): string {
  if (source === "managed") return "Sofvary managed";
  if (source === "external-path") return "External PATH";
  return "Missing";
}

export function formatRuntimeEnvironmentState(
  state: RuntimeEnvironmentInstallState,
): string {
  switch (state) {
    case "installed":
      return "Installed";
    case "installing":
      return "Installing";
    case "failed":
      return "Failed";
    case "unsupported":
      return "Unsupported";
    case "not-installed":
    default:
      return "Not installed";
  }
}

export function getRuntimeEnvironmentActionLabel(
  status: RuntimeEnvironmentStatus,
  version: RuntimeEnvironmentVersionOption | null,
  activeInstallKey: string | null,
): string {
  if (!version) return "Unavailable";
  if (runtimeEnvironmentInstallKey(status, version) === activeInstallKey) return "Installing";
  if (!version.supported || !status.supported) return "Unsupported";
  if (status.installState === "installed" && status.activeVersion === version.version) {
    return "Active";
  }
  if (status.installState === "installed") return "Switch";
  if (status.installState === "failed") return "Retry";
  return "Install";
}

function compareVersionDesc(left: string, right: string): number {
  const leftParts = left.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const rightParts = right.split(".").map((part) => Number.parseInt(part, 10) || 0);
  for (let index = 0; index < Math.max(leftParts.length, rightParts.length); index += 1) {
    const diff = (rightParts[index] ?? 0) - (leftParts[index] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

export function sortRuntimeEnvironmentStatuses(
  statuses: RuntimeEnvironmentStatus[],
): RuntimeEnvironmentStatus[] {
  return [...statuses].sort((left, right) => {
    const stateDiff =
      installStatePriority[left.installState] - installStatePriority[right.installState];
    if (stateDiff !== 0) return stateDiff;
    return left.catalog.label.localeCompare(right.catalog.label);
  });
}
