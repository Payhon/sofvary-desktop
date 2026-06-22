use crate::core::harness_engine::PromptEnvelope;
use crate::core::runtime_diagnostic::RuntimeDiagnostic;

pub fn build_handoff_prompt(envelope: &PromptEnvelope, agent_label: &str) -> String {
    let allowed_files = envelope.output_contract.files.join("\n- ");
    let acceptance = envelope.acceptance_criteria.join("\n- ");
    let prompt_json = serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"# Sofvary Workspace Handoff

You are generating a Sofvary app inside this prepared workspace for {agent_label}.

User intent:
{intent}

Runtime:
- kind: {runtime_kind}
- generated root: {generated_root}
- entrypoint: {entrypoint}
- network: {network}

Required output files:
- {allowed_files}

Acceptance criteria:
- {acceptance}

Rules:
- Write only inside the generated root listed above.
- Generate exactly the required file set unless diagnostics explicitly ask for a repair.
- Do not add Sofvary shell UI, floating menu, build overlay, account, marketplace, or library controls to the generated app.
- Do not read or write outside this workspace.
- Do not run global installs, remote download scripts, or commands that bind to 0.0.0.0.
- When you finish, write files under the generated root. Sofvary will validate and preview them.
- You may append JSON lines to .sofvary/agent-status.jsonl with {{"phase":"...","detail":"..."}}.
- You may write .sofvary/requests/validate.json or .sofvary/requests/preview.json to request Sofvary validation or preview.
- If preview fails, read .sofvary/diagnostics.json and follow .sofvary/repair-prompt.md.

PromptEnvelope:
```json
{prompt_json}
```
"#,
        agent_label = agent_label,
        intent = envelope.user_intent,
        runtime_kind = envelope.box_runtime_context.runtime_kind,
        generated_root = envelope.box_runtime_context.generated_root,
        entrypoint = envelope.box_runtime_context.entrypoint,
        network = envelope.box_runtime_context.network,
        allowed_files = allowed_files,
        acceptance = acceptance,
        prompt_json = prompt_json,
    )
}

pub fn build_repair_prompt(
    original_prompt: &PromptEnvelope,
    diagnostic: &RuntimeDiagnostic,
    attempt: usize,
) -> String {
    let allowed_files = original_prompt.output_contract.files.join("\n- ");
    let diagnostic_json =
        serde_json::to_string_pretty(diagnostic).unwrap_or_else(|_| "{}".to_string());

    format!(
        r#"# Sofvary Repair Handoff

Repair attempt: {attempt}

Original user intent:
{intent}

Runtime diagnostic summary:
{summary}

Required output files:
- {allowed_files}

Repair rules:
- Modify only files inside {generated_root}.
- Keep the same required file set.
- Fix the diagnostic below without adding remote services, Sofvary shell UI, or hidden control chrome.
- After editing, ask Sofvary to validate by writing .sofvary/requests/validate.json.

Diagnostic:
```json
{diagnostic_json}
```
"#,
        attempt = attempt,
        intent = original_prompt.user_intent,
        summary = diagnostic.summary(),
        allowed_files = allowed_files,
        generated_root = original_prompt.box_runtime_context.generated_root,
        diagnostic_json = diagnostic_json,
    )
}

pub fn build_agents_md(envelope: &PromptEnvelope) -> String {
    format!(
        r#"# Sofvary Workspace Agent Instructions

This is a generated-app workspace prepared by Sofvary.

- Work only under `{generated_root}` for generated app source.
- Required files are listed in `.sofvary/allowed-files.json`.
- Runtime contract is in `.sofvary/runtime-contract.json`.
- Full prompt is in `SOFVARY_AGENT_PROMPT.md`.
- Do not modify Sofvary Desktop source, pack caches, lockfiles other than Sofvary-owned metadata, or files outside this workspace.
- Do not include Sofvary shell UI in generated app code.
"#,
        generated_root = envelope.box_runtime_context.generated_root,
    )
}

pub fn build_claude_md(envelope: &PromptEnvelope) -> String {
    format!(
        r#"# Claude Code Instructions

Use `SOFVARY_AGENT_PROMPT.md` as the primary task prompt.

Generate the app only under `{generated_root}`. Keep output within the files listed in `.sofvary/allowed-files.json`.
"#,
        generated_root = envelope.box_runtime_context.generated_root,
    )
}

pub fn build_tools_md() -> String {
    r#"# Sofvary File-Based Control Plane

Sofvary watches this workspace for generated app assets.

Optional files you may write:

- `.sofvary/agent-status.jsonl`: append JSON lines like `{"phase":"generating","detail":"Writing React files"}`.
- `.sofvary/requests/validate.json`: ask Sofvary to re-run contract validation.
- `.sofvary/requests/preview.json`: ask Sofvary to start preview after files are ready.

These files are signals only. Sofvary still validates workspace boundaries, runtime contracts, dependency installs, preview startup, and packaging through its own policy layer.
"#
    .to_string()
}
