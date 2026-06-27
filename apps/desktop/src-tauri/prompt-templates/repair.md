# Sofvary Repair Handoff

Repair attempt: {{repair.attempt}}

Original user intent:
{{user.intent}}

Runtime diagnostic summary:
{{diagnostic.summary}}

Required output files:
{{allowed.filesMarkdown}}

Repair rules:
- Modify only files inside {{runtime.generatedRoot}}.
- Keep the same required file set.
- Fix the diagnostic below without adding remote services, Sofvary shell UI, or hidden control chrome.
- After editing, ask Sofvary to validate by writing .sofvary/requests/validate.json.

Diagnostic:
```json
{{diagnostic.json}}
```
