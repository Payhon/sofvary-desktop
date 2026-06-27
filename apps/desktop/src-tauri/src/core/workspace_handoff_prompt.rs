use crate::core::builtin_resources::get_builtin_resource;
use crate::core::harness_engine::PromptEnvelope;
use crate::core::prompt_template::{render_template, PromptTemplateError};
use crate::core::runtime_diagnostic::RuntimeDiagnostic;
use std::collections::HashMap;

pub fn build_handoff_prompt(
    envelope: &PromptEnvelope,
    agent_label: &str,
) -> Result<String, PromptTemplateError> {
    let prompt_json = serde_json::to_string_pretty(envelope).unwrap_or_else(|_| "{}".to_string());
    let mut variables = envelope_template_variables(envelope);
    variables.insert("agent.label".to_string(), agent_label.to_string());
    variables.insert("prompt.envelopeJson".to_string(), prompt_json);

    render_named_template("prompt-templates/handoff.md", &variables)
}

pub fn build_repair_prompt(
    original_prompt: &PromptEnvelope,
    diagnostic: &RuntimeDiagnostic,
    attempt: usize,
) -> Result<String, PromptTemplateError> {
    let diagnostic_json =
        serde_json::to_string_pretty(diagnostic).unwrap_or_else(|_| "{}".to_string());
    let mut variables = envelope_template_variables(original_prompt);
    variables.insert("repair.attempt".to_string(), attempt.to_string());
    variables.insert("diagnostic.summary".to_string(), diagnostic.summary());
    variables.insert("diagnostic.json".to_string(), diagnostic_json);

    render_named_template("prompt-templates/repair.md", &variables)
}

pub fn build_agents_md(envelope: &PromptEnvelope) -> Result<String, PromptTemplateError> {
    render_named_template(
        "prompt-templates/agents.md",
        &envelope_template_variables(envelope),
    )
}

pub fn build_claude_md(envelope: &PromptEnvelope) -> Result<String, PromptTemplateError> {
    render_named_template(
        "prompt-templates/claude.md",
        &envelope_template_variables(envelope),
    )
}

pub fn build_tools_md() -> Result<String, PromptTemplateError> {
    render_named_template("prompt-templates/tools.md", &HashMap::new())
}

pub fn build_task_md(envelope: &PromptEnvelope) -> Result<String, PromptTemplateError> {
    render_named_template(
        "prompt-templates/task.md",
        &envelope_template_variables(envelope),
    )
}

fn render_named_template(
    path: &str,
    variables: &HashMap<String, String>,
) -> Result<String, PromptTemplateError> {
    let template = get_builtin_resource(path).unwrap_or_else(|| {
        panic!("missing builtin prompt template resource: {path}");
    });
    render_template(template, variables)
}

fn envelope_template_variables(envelope: &PromptEnvelope) -> HashMap<String, String> {
    let mut variables = HashMap::from([
        (
            "runtime.kind".to_string(),
            envelope.box_runtime_context.runtime_kind.clone(),
        ),
        (
            "runtime.id".to_string(),
            envelope.box_runtime_context.runtime_pack.id.clone(),
        ),
        (
            "runtime.version".to_string(),
            envelope.box_runtime_context.runtime_pack.version.clone(),
        ),
        (
            "runtime.generatedRoot".to_string(),
            envelope.box_runtime_context.generated_root.clone(),
        ),
        (
            "runtime.entrypoint".to_string(),
            envelope.box_runtime_context.entrypoint.clone(),
        ),
        (
            "runtime.network".to_string(),
            envelope.box_runtime_context.network.clone(),
        ),
        ("user.intent".to_string(), envelope.user_intent.clone()),
        (
            "workspace.name".to_string(),
            envelope.current_app_state.workspace_name.clone(),
        ),
        (
            "allowed.filesMarkdown".to_string(),
            markdown_list(&envelope.output_contract.files),
        ),
        (
            "acceptance.criteriaMarkdown".to_string(),
            markdown_list(&envelope.acceptance_criteria),
        ),
    ]);
    if let Some(harness) = envelope.box_runtime_context.harness_packs.first() {
        variables.insert("harness.id".to_string(), harness.id.clone());
        variables.insert("harness.version".to_string(), harness.version.clone());
    }
    variables
}

fn markdown_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}
