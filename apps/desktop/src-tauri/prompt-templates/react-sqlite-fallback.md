Software name: {{software.name}}
User requirement:
{{user.intent}}

Runtime repair fallback reason: {{diagnostic.summary}}
Use the Sofvary managed React + SQLite baseline with local CRUD, a Vite frontend, and a local API server.
Derive entities, screens, forms, tables, filters, validation, and seed data from the user requirement; do not fall back to a generic customer manager unless explicitly requested.
Before final validation, perform a short quality loop: review the generated UI/API/schema against the requirement, then revise weak files in place.
Generated React code must avoid unbounded effects, busy loops, tight timers, uncontrolled polling, state updates during render, and heavy animations that can make the preview unresponsive.
Visible app title rule: use only the software name, not the full user requirement, PromptEnvelope text, or runtime repair fallback reason.
