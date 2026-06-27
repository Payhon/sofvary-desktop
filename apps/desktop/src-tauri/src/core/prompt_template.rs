use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PromptTemplateError {
    #[error("template variable '{0}' is missing")]
    MissingVariable(String),
    #[error("template variable name is empty")]
    EmptyVariable,
    #[error("template has an unclosed variable starting at byte {0}")]
    UnclosedVariable(usize),
}

pub type PromptTemplateResult<T> = Result<T, PromptTemplateError>;

pub fn render_template(
    template: &str,
    variables: &HashMap<String, String>,
) -> PromptTemplateResult<String> {
    let mut output = String::with_capacity(template.len());
    let mut index = 0;

    while let Some(start_offset) = template[index..].find("{{") {
        let start = index + start_offset;
        output.push_str(&template[index..start]);
        let variable_start = start + 2;
        let Some(end_offset) = template[variable_start..].find("}}") else {
            return Err(PromptTemplateError::UnclosedVariable(start));
        };
        let end = variable_start + end_offset;
        let variable = template[variable_start..end].trim();
        if variable.is_empty() {
            return Err(PromptTemplateError::EmptyVariable);
        }
        let value = variables
            .get(variable)
            .ok_or_else(|| PromptTemplateError::MissingVariable(variable.to_string()))?;
        output.push_str(value);
        index = end + 2;
    }

    output.push_str(&template[index..]);
    Ok(output)
}

pub fn render_template_list(
    values: &[String],
    variables: &HashMap<String, String>,
) -> PromptTemplateResult<Vec<String>> {
    values
        .iter()
        .map(|value| render_template(value, variables))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_known_variables() {
        let variables = HashMap::from([
            ("runtime.kind".to_string(), "react-vite".to_string()),
            ("workspace.name".to_string(), "Tasks".to_string()),
        ]);

        let rendered = render_template(
            "Build {{workspace.name}} with {{ runtime.kind }}.",
            &variables,
        )
        .expect("render");

        assert_eq!(rendered, "Build Tasks with react-vite.");
    }

    #[test]
    fn rejects_missing_variables() {
        let error = render_template("{{user.intent}}", &HashMap::new()).expect_err("missing");

        assert!(error.to_string().contains("user.intent"));
    }
}
