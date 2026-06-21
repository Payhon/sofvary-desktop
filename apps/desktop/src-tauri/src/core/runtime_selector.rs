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

#[derive(Debug, Clone, Copy)]
struct RuntimeRule {
    runtime_kind: RuntimeKind,
    software_type: &'static str,
    reason: &'static str,
    signals: &'static [&'static str],
    weight: i32,
}

const RULES: &[RuntimeRule] = &[
    RuntimeRule {
        runtime_kind: RuntimeKind::AiAgentApp,
        software_type: "AI Agent App",
        reason: "The request is about creating a user-owned AI Agent app with provider binding, AI job calls, or generated text, image, or video artifacts.",
        signals: &[
            "ai agent",
            "agent app",
            "provider binding",
            "llm provider",
            "write article",
            "article agent",
            "write novel",
            "novel agent",
            "image generation",
            "generate image",
            "video generation",
            "generate video",
            "multimodal",
            "生图",
            "生成图片",
            "生成视频",
            "写文章",
            "写小说",
            "智能体",
            "智能体应用",
        ],
        weight: 6,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::FileProcessor,
        software_type: "File Processor",
        reason: "The request is about explicit local files, batch operations, or safe dry-run processing.",
        signals: &[
            "batch rename",
            "rename files",
            "folder",
            "files",
            "file processor",
            "organize images",
            "批量重命名",
            "重命名",
            "文件处理",
            "整理文件",
            "文件夹",
            "图片整理",
        ],
        weight: 5,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::ReactSqlite,
        software_type: "Local Data App",
        reason: "The request needs records that can be created, edited, searched, and kept over time.",
        signals: &[
            "customer",
            "crm",
            "contact",
            "order",
            "status",
            "follow-up",
            "follow up",
            "database",
            "sqlite",
            "persist",
            "reminder",
            "客户",
            "联系人",
            "订单",
            "跟进",
            "状态",
            "提醒",
            "长期保存",
            "持久",
            "数据库",
            "增删改查",
        ],
        weight: 5,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::DataTable,
        software_type: "Data Table Tool",
        reason: "The request is centered on tabular data, filtering, sorting, CSV, or lightweight data cleaning.",
        signals: &[
            "table",
            "spreadsheet",
            "csv",
            "inventory",
            "budget",
            "filter",
            "sort",
            "data cleaning",
            "data cleanup",
            "表格",
            "数据表",
            "CSV",
            "库存",
            "预算",
            "筛选",
            "排序",
            "数据清洗",
            "清洗页面",
        ],
        weight: 4,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::MarkdownKnowledge,
        software_type: "Knowledge App",
        reason: "The request is about notes, markdown, documents, search, or a personal knowledge base.",
        signals: &[
            "markdown",
            "notes",
            "wiki",
            "knowledge",
            "document",
            "reading",
            "prompt library",
            "笔记",
            "知识库",
            "文档",
            "阅读",
            "写作",
            "提示词库",
            "本地 wiki",
        ],
        weight: 4,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::Canvas2d,
        software_type: "Interactive Visual Tool",
        reason: "The request needs canvas-style drawing, animation, game loops, or visual simulation.",
        signals: &[
            "canvas",
            "game",
            "animation",
            "simulation",
            "draw",
            "visual",
            "游戏",
            "动画",
            "画布",
            "绘图",
            "模拟",
            "可视化",
        ],
        weight: 4,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::DesktopWidget,
        software_type: "Desktop Widget",
        reason: "The request sounds like a compact timer, countdown, quick note, or small desktop utility.",
        signals: &[
            "pomodoro",
            "timer",
            "countdown",
            "widget",
            "quick note",
            "small dashboard",
            "番茄钟",
            "计时",
            "倒计时",
            "小组件",
            "便签",
            "桌面组件",
            "小面板",
        ],
        weight: 4,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::ReactVite,
        software_type: "Interactive App",
        reason: "The request asks for a richer interactive UI without a clear local database or file-processing need.",
        signals: &[
            "dashboard",
            "kanban",
            "task board",
            "interactive",
            "multi page",
            "form",
            "面板",
            "看板",
            "任务板",
            "交互",
            "多页面",
            "表单",
        ],
        weight: 3,
    },
    RuntimeRule {
        runtime_kind: RuntimeKind::StaticHtml,
        software_type: "Lightweight Page Tool",
        reason: "The request can be served as a small single-page tool without package installation or local data services.",
        signals: &[
            "landing page",
            "single page",
            "calculator",
            "simple tool",
            "static",
            "页面",
            "单页",
            "计算器",
            "轻量",
            "简单工具",
            "静态",
        ],
        weight: 2,
    },
];

pub fn select_runtime_for_intent(intent: &str) -> RuntimeIntentSelection {
    let normalized = intent.to_lowercase();
    let mut scores = Vec::new();

    for rule in RULES {
        let mut score = 0;
        let mut matched = Vec::new();
        for signal in rule.signals {
            if normalized.contains(&signal.to_lowercase()) {
                score += rule.weight;
                matched.push((*signal).to_string());
            }
        }
        scores.push((rule, score, matched));
    }

    scores.sort_by(|left, right| {
        right.1.cmp(&left.1).then_with(|| {
            runtime_priority(right.0.runtime_kind).cmp(&runtime_priority(left.0.runtime_kind))
        })
    });

    let (selected_rule, selected_score, matched_signals) = match scores.first().cloned() {
        Some((_, 0, _)) | None => (static_html_rule(), 0, Vec::new()),
        Some(selection) => selection,
    };
    let alternatives = scores
        .iter()
        .filter(|(rule, score, _)| rule.runtime_kind != selected_rule.runtime_kind && *score > 0)
        .take(2)
        .map(|(rule, _, _)| rule.runtime_kind)
        .collect::<Vec<_>>();

    RuntimeIntentSelection {
        runtime_kind: selected_rule.runtime_kind,
        software_type: selected_rule.software_type.to_string(),
        confidence: confidence_for_score(selected_score),
        reason: selected_rule.reason.to_string(),
        matched_signals,
        alternatives,
        source: RuntimeSelectionSource::Automatic,
    }
}

pub fn manual_runtime_selection(runtime_kind: RuntimeKind) -> RuntimeIntentSelection {
    let rule = RULES
        .iter()
        .find(|rule| rule.runtime_kind == runtime_kind)
        .unwrap_or(&RULES[RULES.len() - 1]);

    RuntimeIntentSelection {
        runtime_kind,
        software_type: rule.software_type.to_string(),
        confidence: 1.0,
        reason: "The user selected this runtime explicitly.".to_string(),
        matched_signals: Vec::new(),
        alternatives: Vec::new(),
        source: RuntimeSelectionSource::Manual,
    }
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

fn runtime_priority(runtime_kind: RuntimeKind) -> i32 {
    match runtime_kind {
        RuntimeKind::AiAgentApp => 90,
        RuntimeKind::ReactSqlite => 80,
        RuntimeKind::DataTable => 70,
        RuntimeKind::FileProcessor => 65,
        RuntimeKind::MarkdownKnowledge => 60,
        RuntimeKind::Canvas2d => 55,
        RuntimeKind::DesktopWidget => 50,
        RuntimeKind::ReactVite => 40,
        RuntimeKind::StaticHtml => 10,
    }
}

fn static_html_rule() -> &'static RuntimeRule {
    RULES
        .iter()
        .find(|rule| rule.runtime_kind == RuntimeKind::StaticHtml)
        .unwrap_or(&RULES[RULES.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_react_sqlite_for_customer_follow_up() {
        let selection =
            select_runtime_for_intent("创建一个客户跟进小面板，记录联系人状态和下次提醒");

        assert_eq!(selection.runtime_kind, RuntimeKind::ReactSqlite);
        assert!(selection.confidence >= 0.84);
    }

    #[test]
    fn selects_data_table_for_lightweight_data_cleaning() {
        let selection = select_runtime_for_intent("做一个轻量数据清洗页面，支持 CSV 筛选排序");

        assert_eq!(selection.runtime_kind, RuntimeKind::DataTable);
    }

    #[test]
    fn selects_ai_agent_app_for_multimodal_agent() {
        let selection = select_runtime_for_intent(
            "Create an AI agent app that can write articles and generate video",
        );

        assert_eq!(selection.runtime_kind, RuntimeKind::AiAgentApp);
        assert!(selection.confidence >= 0.84);
    }

    #[test]
    fn selects_desktop_widget_for_countdown() {
        let selection = select_runtime_for_intent("生成一个本地倒计时工具");

        assert_eq!(selection.runtime_kind, RuntimeKind::DesktopWidget);
    }

    #[test]
    fn falls_back_to_static_html_when_unclear() {
        let selection = select_runtime_for_intent("hello");

        assert_eq!(selection.runtime_kind, RuntimeKind::StaticHtml);
        assert!(selection.confidence < 0.6);
    }

    #[test]
    fn manual_selection_marks_source() {
        let selection = manual_runtime_selection(RuntimeKind::Canvas2d);

        assert_eq!(selection.runtime_kind, RuntimeKind::Canvas2d);
        assert_eq!(selection.source, RuntimeSelectionSource::Manual);
        assert_eq!(selection.confidence, 1.0);
    }
}
