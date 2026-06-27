# Sofvary File-Based Control Plane

Sofvary watches this workspace for generated app assets.

Optional files you may write:

- `.sofvary/agent-status.jsonl`: append JSON lines like `{"phase":"generating","detail":"Writing React files"}`.
- `.sofvary/requests/validate.json`: ask Sofvary to re-run contract validation.
- `.sofvary/requests/preview.json`: ask Sofvary to start preview after files are ready.

These files are signals only. Sofvary still validates workspace boundaries, runtime contracts, dependency installs, preview startup, and packaging through its own policy layer.
