import { useEffect, useMemo, useState } from "react";
import { AlertCircle, CheckCircle2, FolderOpen, Image, Loader2, PackageCheck, RefreshCw, X } from "lucide-react";
import type {
  AppReleaseCapability,
  AppReleaseStealthUiSettings,
  AppReleaseTargetPlatform,
  PackagerToolchainStatus,
  RuntimeKind,
  WorkspaceSummary,
} from "../../types";
import {
  buildReleaseDefaultName,
  canStartRelease,
  DEFAULT_RELEASE_STEALTH_UI_SETTINGS,
  getCurrentReleasePlatform,
  getReleasePlatformReason,
  runtimeReleaseCapability,
  type ReleaseStatusState,
} from "../../core/release/releaseLogic";
import { useDesktopLocale } from "../i18n/DesktopLocaleProvider";

interface ReleaseWizardSubmitInput {
  appName: string;
  targetPlatform: AppReleaseTargetPlatform;
  outputDir: string;
  iconPath: string | null;
  includeAiContinuation: boolean;
  stealthUi: AppReleaseStealthUiSettings;
}

interface ReleaseWizardProps {
  workspace: WorkspaceSummary;
  capabilities: AppReleaseCapability | null;
  toolchainStatus: PackagerToolchainStatus | null;
  releaseStatus: ReleaseStatusState;
  statusLine: string;
  busy: boolean;
  onClose: () => void;
  onRefreshToolchain: () => void;
  onInstallToolchain: () => void;
  onSelectOutputFolder: () => Promise<string | null>;
  onSelectIcon: () => Promise<string | null>;
  onSubmit: (input: ReleaseWizardSubmitInput) => void;
}

const releasePlatforms: AppReleaseTargetPlatform[] = ["windows", "macos", "linux"];

export function ReleaseWizard({
  workspace,
  capabilities,
  toolchainStatus,
  releaseStatus,
  statusLine,
  busy,
  onClose,
  onRefreshToolchain,
  onInstallToolchain,
  onSelectOutputFolder,
  onSelectIcon,
  onSubmit,
}: ReleaseWizardProps) {
  const { t } = useDesktopLocale();
  const [appName, setAppName] = useState(() => buildReleaseDefaultName(workspace));
  const [targetPlatform, setTargetPlatform] = useState<AppReleaseTargetPlatform>(() =>
    getCurrentReleasePlatform(capabilities),
  );
  const [outputDir, setOutputDir] = useState("");
  const [iconPath, setIconPath] = useState<string | null>(null);

  useEffect(() => {
    setAppName(buildReleaseDefaultName(workspace));
    setTargetPlatform(getCurrentReleasePlatform(capabilities));
    setOutputDir("");
    setIconPath(null);
  }, [capabilities, workspace.appId, workspace.name]);

  const runtimeCapability = useMemo(
    () => runtimeReleaseCapability(capabilities, workspace.mode),
    [capabilities, workspace.mode],
  );
  const startEnabled = canStartRelease({
    appName,
    outputDir,
    targetPlatform,
    capabilities,
    toolchainStatus,
    busy,
  });
  const isPublishing = releaseStatus.kind === "publishing";
  const isSuccess = releaseStatus.kind === "success";
  const isError = releaseStatus.kind === "error";

  return (
    <section className="release-wizard" role="dialog" aria-modal="true" data-no-drag>
      <div className="release-wizard__surface">
        <header className="release-wizard__header">
          <div>
            <span>{t("release.eyebrow", {}, "Publish app")}</span>
            <h2>{workspace.name}</h2>
            <small>{statusLine}</small>
          </div>
          <button type="button" aria-label={t("action.cancel")} onClick={onClose}>
            <X aria-hidden="true" />
          </button>
        </header>

        <div className="release-wizard__body">
          {isPublishing ? (
            <div className="release-wizard__state release-wizard__state--busy">
              <Loader2 aria-hidden="true" />
              <strong>{t("release.buildingTitle", {}, "正在构建发布包")}</strong>
              <p>{t("release.buildingDetail", {}, "Sofvary 正在生成可运行的应用和安装包，完成后会打开输出文件夹。")}</p>
              <small>{statusLine}</small>
            </div>
          ) : isSuccess ? (
            <div className="release-wizard__state release-wizard__state--success">
              <CheckCircle2 aria-hidden="true" />
              <strong>{t("release.successTitle", {}, "软件生成成功")}</strong>
              <p>{t("release.successDetail", {}, "发布产物已写入输出目录，文件夹已自动打开。")}</p>
              <small>{releaseStatus.detail ?? statusLine}</small>
            </div>
          ) : (
            <>
              {isError ? (
                <div className="release-wizard__alert" role="alert">
                  <AlertCircle aria-hidden="true" />
                  <span>{statusLine}</span>
                </div>
              ) : null}
              <section className="release-section" aria-label={t("release.platform", {}, "Target platform")}>
                <div className="release-section__heading">
                  <strong>{t("release.platform", {}, "Target platform")}</strong>
                  <small>{t("release.platformDetail", {}, "First beta supports local OS packaging only.")}</small>
                </div>
                <div className="release-platforms" role="radiogroup" aria-label={t("release.platform", {}, "Target platform")}>
                  {releasePlatforms.map((platform) => {
                    const capability = capabilities?.targetPlatforms.find((item) => item.platform === platform);
                    const enabled = capability?.enabled ?? false;
                    return (
                      <button
                        key={platform}
                        type="button"
                        role="radio"
                        aria-checked={targetPlatform === platform}
                        className={targetPlatform === platform ? "is-active" : ""}
                        disabled={!enabled || busy}
                        title={capability?.reason ?? getReleasePlatformReason(capabilities, platform)}
                        onClick={() => setTargetPlatform(platform)}
                      >
                        <strong>{capability?.label ?? platform}</strong>
                        <small>{capability?.outputKind ?? "Beta package"}</small>
                        {!enabled ? <span>{capability?.reason ?? "本机发布仅支持当前 OS"}</span> : null}
                      </button>
                    );
                  })}
                </div>
              </section>

              <section className="release-section release-form-grid" aria-label={t("release.details", {}, "Release details")}>
                <label>
                  <span>{t("release.appName", {}, "App name")}</span>
                  <input
                    value={appName}
                    disabled={busy}
                    onChange={(event) => setAppName(event.currentTarget.value)}
                  />
                </label>
                <div className="release-file-picker">
                  <span>{t("release.icon", {}, "Icon")}</span>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={async () => {
                      const selected = await onSelectIcon();
                      if (selected) setIconPath(selected);
                    }}
                  >
                    <Image aria-hidden="true" />
                    {iconPath ? t("release.changeIcon", {}, "Change icon") : t("release.useDefaultIcon", {}, "Use Sofvary icon")}
                  </button>
                  <small>{iconPath ?? t("release.defaultIconDetail", {}, "Default Sofvary icon will be used.")}</small>
                  <small className="release-file-picker__hint">
                    {t("release.iconSpec", {}, "PNG must be square and at least 512x512 px. macOS accepts .icns; Windows accepts .ico.")}
                  </small>
                </div>
                <div className="release-file-picker release-file-picker--wide">
                  <span>{t("release.outputFolder", {}, "Output folder")}</span>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={async () => {
                      const selected = await onSelectOutputFolder();
                      if (selected) setOutputDir(selected);
                    }}
                  >
                    <FolderOpen aria-hidden="true" />
                    {outputDir ? t("release.changeOutput", {}, "Change folder") : t("release.chooseOutput", {}, "Choose folder")}
                  </button>
                  <small>{outputDir || t("release.outputRequired", {}, "Select where the published package will be written.")}</small>
                </div>
              </section>

              <section className="release-section release-runtime" aria-label={t("release.runtime", {}, "Runtime")}>
                <div className="release-section__heading">
                  <strong>{runtimeCapability?.label ?? formatRuntimeKind(workspace.mode)}</strong>
                  <small>{runtimeCapability?.releaseStrategy ?? workspace.mode}</small>
                </div>
                <div className="release-pack-items" aria-label={t("release.packItems", {}, "Package contents")}>
                  <label>
                    <input type="checkbox" checked readOnly />
                    <span>{t("release.includeRuntime", {}, "Runtime metadata and lockfile")}</span>
                  </label>
                  <label>
                    <input type="checkbox" checked readOnly />
                    <span>{t("release.includeEnvironment", {}, "Packager environment manifest")}</span>
                  </label>
                  <label>
                    <input type="checkbox" checked readOnly />
                    <span>{t("release.includePlugins", {}, "Plugin metadata lockfile")}</span>
                  </label>
                </div>
                <div className="release-runtime__notes">
                  {(runtimeCapability?.notes ?? ["Runtime metadata and lockfile will be packaged."]).map((note) => (
                    <span key={note}>{note}</span>
                  ))}
                </div>
              </section>

              <section className="release-section release-toolchain" aria-label={t("release.toolchain", {}, "Packager toolchain")}>
                <div className="release-section__heading">
                  <strong>{t("release.toolchain", {}, "Packager toolchain")}</strong>
                  <small>{toolchainStatus?.detail ?? t("status.loading")}</small>
                </div>
                <div className="release-toolchain__requirements">
                  {(toolchainStatus?.requirements ?? []).map((requirement) => (
                    <article key={requirement.kind} className={requirement.installed ? "is-ready" : ""}>
                      <CheckCircle2 aria-hidden="true" />
                      <span>{requirement.label}</span>
                      <small>{requirement.version ?? requirement.detail}</small>
                    </article>
                  ))}
                </div>
                <div className="release-toolchain__actions">
                  <button type="button" disabled={busy} onClick={onRefreshToolchain}>
                    <RefreshCw aria-hidden="true" />
                    {t("action.refresh")}
                  </button>
                  <button
                    type="button"
                    disabled={busy || !toolchainStatus?.installActionAvailable}
                    onClick={onInstallToolchain}
                  >
                    {t("action.install")}
                  </button>
                </div>
              </section>
            </>
          )}
        </div>

        <footer className={isSuccess ? "release-wizard__actions release-wizard__actions--success" : "release-wizard__actions"}>
          {!isSuccess ? (
            <button type="button" disabled={busy} onClick={onClose}>
              {t("action.cancel")}
            </button>
          ) : null}
          {isSuccess ? (
            <button type="button" className="release-wizard__primary" onClick={onClose}>
              <CheckCircle2 aria-hidden="true" />
              {t("action.done", {}, "Done")}
            </button>
          ) : (
            <button
              type="button"
              className="release-wizard__primary"
              disabled={!startEnabled}
              onClick={() =>
                onSubmit({
                  appName,
                  targetPlatform,
                  outputDir,
                  iconPath,
                  includeAiContinuation: false,
                  stealthUi: DEFAULT_RELEASE_STEALTH_UI_SETTINGS,
                })
              }
            >
              {isPublishing ? <Loader2 aria-hidden="true" /> : <PackageCheck aria-hidden="true" />}
              {busy ? t("status.working") : t("release.start", {}, "Publish")}
            </button>
          )}
        </footer>
      </div>
    </section>
  );
}

function formatRuntimeKind(runtimeKind: RuntimeKind): string {
  return runtimeKind
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}
