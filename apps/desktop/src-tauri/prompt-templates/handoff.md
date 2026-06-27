# Sofvary Workspace Handoff

You are generating a Sofvary app inside this prepared workspace for {{agent.label}}.

User intent:
{{user.intent}}

Runtime:
- kind: {{runtime.kind}}
- generated root: {{runtime.generatedRoot}}
- entrypoint: {{runtime.entrypoint}}
- network: {{runtime.network}}

Required output files:
{{allowed.filesMarkdown}}

Acceptance criteria:
{{acceptance.criteriaMarkdown}}

Rules:
- Write only inside the generated root listed above.
- Generate exactly the required file set unless diagnostics explicitly ask for a repair.
- Do not add Sofvary shell UI, floating menu, build overlay, account, marketplace, or library controls to the generated app.
- Do not read or write outside this workspace.
- Do not run global installs, remote download scripts, or commands that bind to 0.0.0.0.
- When you finish, write files under the generated root. Sofvary will validate and preview them.
- You may append JSON lines to .sofvary/agent-status.jsonl with {"phase":"...","detail":"..."}.
- You may write .sofvary/requests/validate.json or .sofvary/requests/preview.json to request Sofvary validation or preview.
- If preview fails, read .sofvary/diagnostics.json and follow .sofvary/repair-prompt.md.

PromptEnvelope:
```json
{{prompt.envelopeJson}}
```
