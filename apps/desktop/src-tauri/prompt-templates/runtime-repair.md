Repair the generated Sofvary app so it starts successfully.
Software name: {{software.name}}
Original user intent:
{{user.intent}}

Runtime kind: {{runtime.kind}}
Repair attempt: {{repair.attempt}}/{{repair.maxAttempts}}
Failed stage: {{diagnostic.stage}}
Failed command: {{diagnostic.command}}
Status code: {{diagnostic.statusCode}}
Diagnostic category: {{diagnostic.category}}
Runtime log path: {{diagnostic.logPath}}

stdout tail:
{{diagnostic.stdoutTail}}

stderr tail:
{{diagnostic.stderrTail}}

Keep the same output contract and regenerate every required file exactly. Do not add files outside the allowed set. Do not include Sofvary shell UI. The visible app title must stay a concise software name and must not display this repair prompt or diagnostics.
