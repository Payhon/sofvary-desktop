use crate::core::pack_manager::runtime_catalog_manifests;
use crate::core::workspace_types::RuntimeKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIntentSelection {
    pub runtime_kind: RuntimeKind,
    pub software_type: String,
    pub confidence: f32,
    pub reason: String,
    pub matched_signals: Vec<String>,
    pub alternatives: Vec<RuntimeKind>,
    pub source: RuntimeSelectionSource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeSelectionSource {
    Automatic,
    Manual,
}

#[derive(Debug, Clone)]
struct RuntimeRule {
    runtime_kind: RuntimeKind,
    software_type: String,
    reason: String,
    signals: Vec<String>,
    weight: i32,
    priority: i32,
}

pub fn select_runtime_for_intent(intent: &str) -> RuntimeIntentSelection {
    let rules = runtime_rules();
    let normalized = intent.to_lowercase();
    let mut scores = Vec::new();

    for rule in &rules {
        let mut score = 0;
        let mut matched = Vec::new();
        for signal in &rule.signals {
            if normalized.contains(&signal.to_lowercase()) {
                score += rule.weight;
                matched.push(signal.clone());
            }
        }
        scores.push((rule, score, matched));
    }

    scores.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.0.priority.cmp(&left.0.priority))
    });

    let (selected_rule, selected_score, matched_signals) = match scores.first().cloned() {
        Some((_, 0, _)) | None => (static_html_rule(&rules), 0, Vec::new()),
        Some(selection) => selection,
    };
    let alternatives = scores
        .iter()
        .filter(|(rule, score, _)| rule.runtime_kind != selected_rule.runtime_kind && *score > 0)
        .take(2)
        .map(|(rule, _, _)| rule.runtime_kind.clone())
        .collect::<Vec<_>>();

    RuntimeIntentSelection {
        runtime_kind: selected_rule.runtime_kind.clone(),
        software_type: selected_rule.software_type.clone(),
        confidence: confidence_for_score(selected_score),
        reason: selected_rule.reason.clone(),
        matched_signals,
        alternatives,
        source: RuntimeSelectionSource::Automatic,
    }
}

pub fn manual_runtime_selection(runtime_kind: RuntimeKind) -> RuntimeIntentSelection {
    let rules = runtime_rules();
    let fallback = static_html_rule(&rules);
    let rule = rules
        .iter()
        .find(|rule| rule.runtime_kind == runtime_kind)
        .unwrap_or(fallback);

    RuntimeIntentSelection {
        runtime_kind,
        software_type: rule.software_type.clone(),
        confidence: 1.0,
        reason: "The user selected this runtime explicitly.".to_string(),
        matched_signals: Vec::new(),
        alternatives: Vec::new(),
        source: RuntimeSelectionSource::Manual,
    }
}

fn runtime_rules() -> Vec<RuntimeRule> {
    let mut rules = runtime_catalog_manifests()
        .unwrap_or_default()
        .into_iter()
        .map(|manifest| RuntimeRule {
            runtime_kind: manifest.runtime.kind,
            software_type: manifest.selection.software_type,
            reason: manifest.selection.reason,
            signals: manifest.selection.signals,
            weight: manifest.selection.weight,
            priority: manifest.selection.priority,
        })
        .collect::<Vec<_>>();
    rules.sort_by(|left, right| right.priority.cmp(&left.priority));
    rules
}

fn confidence_for_score(score: i32) -> f32 {
    match score {
        score if score >= 10 => 0.94,
        score if score >= 8 => 0.9,
        score if score >= 5 => 0.84,
        score if score >= 3 => 0.72,
        score if score >= 2 => 0.64,
        _ => 0.52,
    }
}

fn static_html_rule(rules: &[RuntimeRule]) -> &RuntimeRule {
    rules
        .iter()
        .max_by_key(|rule| rule.priority)
        .unwrap_or_else(|| {
            rules
                .first()
                .expect("runtime catalog must include at least one runtime")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_react_sqlite_for_customer_follow_up() {
        let selection =
            select_runtime_for_intent("创建一个客户跟进小面板，记录联系人状态和下次提醒");

        assert_eq!(selection.runtime_kind, "react-sqlite");
        assert!(selection.confidence >= 0.84);
    }

    #[test]
    fn selects_data_table_for_lightweight_data_cleaning() {
        let selection = select_runtime_for_intent("做一个轻量数据清洗页面，支持 CSV 筛选排序");

        assert_eq!(selection.runtime_kind, "data-table");
    }

    #[test]
    fn selects_ai_agent_app_for_multimodal_agent() {
        let selection = select_runtime_for_intent(
            "Create an AI agent app that can write articles and generate video",
        );

        assert_eq!(selection.runtime_kind, "ai-agent-app");
        assert!(selection.confidence >= 0.84);
    }

    #[test]
    fn selects_desktop_widget_for_countdown() {
        let selection = select_runtime_for_intent("生成一个本地倒计时工具");

        assert_eq!(selection.runtime_kind, "desktop-widget");
    }

    #[test]
    fn falls_back_to_static_html_when_unclear() {
        let selection = select_runtime_for_intent("hello");

        assert_eq!(
            selection.runtime_kind,
            static_html_rule(&runtime_rules()).runtime_kind
        );
        assert!(selection.confidence < 0.6);
    }

    #[test]
    fn manual_selection_marks_source() {
        let selection = manual_runtime_selection("canvas2d".to_string());

        assert_eq!(selection.runtime_kind, "canvas2d");
        assert_eq!(selection.source, RuntimeSelectionSource::Manual);
        assert_eq!(selection.confidence, 1.0);
    }
}
