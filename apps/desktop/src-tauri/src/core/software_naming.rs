const DEFAULT_SOFTWARE_NAME: &str = "Untitled App";
const DEFAULT_CHINESE_SOFTWARE_NAME: &str = "新软件";

pub fn suggest_software_name(requirement: &str) -> String {
    if let Some(explicit_name) = extract_explicit_software_name(requirement) {
        return clean_display_name(&explicit_name, DEFAULT_SOFTWARE_NAME, 32);
    }

    let normalized = normalize_visible_text(requirement);
    if normalized.is_empty() {
        return DEFAULT_SOFTWARE_NAME.to_string();
    }

    if let Some(explicit_name) = extract_explicit_software_name(&normalized) {
        return clean_display_name(&explicit_name, DEFAULT_SOFTWARE_NAME, 32);
    }

    if contains_cjk(&normalized) {
        if let Some(name) = chinese_domain_name(&normalized) {
            return name.to_string();
        }
        return fallback_chinese_name(&normalized);
    }

    if let Some(name) = english_domain_name(&normalized) {
        return name.to_string();
    }

    fallback_english_name(&normalized)
}

pub fn clean_display_name(name: &str, fallback: &str, max_chars: usize) -> String {
    let cleaned = normalize_visible_text(name);
    if cleaned.is_empty() {
        return fallback.to_string();
    }

    let shortened: String = cleaned.chars().take(max_chars).collect();
    let shortened = shortened.trim_matches(['-', ':', '：', ',', '，', '.', '。', ' ']);
    if shortened.is_empty() {
        fallback.to_string()
    } else {
        shortened.to_string()
    }
}

fn normalize_visible_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_explicit_software_name(value: &str) -> Option<String> {
    for line in value.lines() {
        let line = line.trim();
        for marker in [
            "Software name:",
            "App name:",
            "Application name:",
            "软件名:",
            "软件名称:",
            "应用名:",
            "应用名称:",
            "Software name：",
            "App name：",
            "软件名：",
            "软件名称：",
            "应用名：",
            "应用名称：",
        ] {
            if let Some(rest) = line.strip_prefix(marker) {
                let candidate = trim_after_name_value(rest.trim());
                if !candidate.is_empty() {
                    return Some(candidate.to_string());
                }
            }
        }
    }
    None
}

fn trim_after_name_value(value: &str) -> &str {
    let markers = [
        " User requirement:",
        " Original user intent:",
        " Runtime kind:",
        " Runtime repair fallback reason:",
        " 用户需求:",
        " 原始需求:",
        " 修复原因:",
    ];
    let end = markers
        .iter()
        .filter_map(|marker| value.find(marker))
        .min()
        .unwrap_or(value.len());
    value[..end].trim()
}

fn contains_cjk(value: &str) -> bool {
    value.chars().any(|ch| {
        ('\u{4e00}'..='\u{9fff}').contains(&ch) || ('\u{3400}'..='\u{4dbf}').contains(&ch)
    })
}

fn chinese_domain_name(value: &str) -> Option<&'static str> {
    let pairs: &[(&[&str], &str)] = &[
        (&["排课", "课程", "教室", "老师"], "排课助手"),
        (&["课表", "课程表"], "课表管家"),
        (&["学员", "学生"], "学员管家"),
        (&["客户", "跟进"], "客户管家"),
        (&["客户", "crm"], "客户管家"),
        (&["库存", "进销存"], "库存管家"),
        (&["订单", "销售"], "订单管家"),
        (&["任务", "待办"], "任务看板"),
        (&["倒计时", "计时"], "计时器"),
        (&["番茄", "专注"], "专注计时"),
        (&["文件", "重命名"], "文件整理器"),
        (&["数据", "清洗"], "数据清洗台"),
        (&["知识", "笔记"], "知识库"),
        (&["预算", "记账"], "记账助手"),
        (&["报表", "统计"], "报表台"),
    ];

    pairs
        .iter()
        .find(|(needles, _)| needles.iter().any(|needle| value.contains(needle)))
        .map(|(_, name)| *name)
}

fn english_domain_name(value: &str) -> Option<&'static str> {
    let lower = value.to_ascii_lowercase();
    let pairs: &[(&[&str], &str)] = &[
        (&["schedule", "class"], "Schedule Studio"),
        (&["course", "teacher"], "Course Planner"),
        (&["student", "course"], "Course Planner"),
        (&["customer", "crm"], "Customer Desk"),
        (&["customer", "follow"], "Customer Desk"),
        (&["inventory", "stock"], "Inventory Desk"),
        (&["order", "sales"], "Order Desk"),
        (&["task", "todo"], "Task Board"),
        (&["countdown", "timer"], "Timer"),
        (&["pomodoro", "focus"], "Focus Timer"),
        (&["file", "rename"], "File Organizer"),
        (&["data", "clean"], "Data Cleaner"),
        (&["tiny notes"], "Tiny Notes"),
        (&["reading notes"], "Reading Notes"),
        (&["knowledge", "notes"], "Knowledge Base"),
        (&["budget", "expense"], "Budget Desk"),
        (&["report", "analytics"], "Report Desk"),
    ];

    pairs
        .iter()
        .find(|(needles, _)| needles.iter().any(|needle| lower.contains(needle)))
        .map(|(_, name)| *name)
}

fn fallback_chinese_name(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if !is_cjk_content_char(ch) {
            continue;
        }
        if is_cjk_stop_char(ch) {
            continue;
        }
        result.push(ch);
        if result.chars().count() >= 6 {
            break;
        }
    }

    if result.chars().count() >= 2 {
        result
    } else {
        DEFAULT_CHINESE_SOFTWARE_NAME.to_string()
    }
}

fn fallback_english_name(value: &str) -> String {
    let skip = [
        "a",
        "an",
        "the",
        "this",
        "that",
        "for",
        "to",
        "of",
        "with",
        "and",
        "or",
        "create",
        "build",
        "make",
        "generate",
        "stable",
        "app",
        "application",
        "software",
        "tool",
        "requirement",
    ];
    let mut words = Vec::new();
    for word in value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        let lower = word.to_ascii_lowercase();
        if skip.contains(&lower.as_str()) {
            continue;
        }
        words.push(title_case_ascii(word));
        if words.len() >= 3 {
            break;
        }
    }

    if words.is_empty() {
        DEFAULT_SOFTWARE_NAME.to_string()
    } else {
        clean_display_name(&words.join(" "), DEFAULT_SOFTWARE_NAME, 32)
    }
}

fn title_case_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!(
        "{}{}",
        first.to_ascii_uppercase(),
        chars.as_str().to_ascii_lowercase()
    )
}

fn is_cjk_content_char(ch: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&ch) || ('\u{3400}'..='\u{4dbf}').contains(&ch)
}

fn is_cjk_stop_char(ch: char) -> bool {
    matches!(
        ch,
        '生' | '成'
            | '创'
            | '建'
            | '做'
            | '一'
            | '个'
            | '款'
            | '软'
            | '件'
            | '系'
            | '统'
            | '应'
            | '用'
            | '工'
            | '具'
            | '需'
            | '要'
            | '实'
            | '现'
            | '我'
            | '的'
            | '当'
            | '前'
            | '本'
            | '地'
            | '所'
            | '有'
            | '信'
            | '息'
            | '增'
            | '删'
            | '改'
            | '查'
            | '可'
            | '以'
            | '和'
            | '与'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_short_chinese_domain_name() {
        let requirement = "生成一个排课管理软件，有学员名单管理，课程管理，老师管理，教室管理";

        assert_eq!(suggest_software_name(requirement), "排课助手");
    }

    #[test]
    fn extracts_explicit_agent_name_from_repair_prompt() {
        let prompt = "Software name: 排课助手\nUser requirement:\n生成一个排课管理软件\nRuntime repair fallback reason: local API failed";

        assert_eq!(suggest_software_name(prompt), "排课助手");
    }

    #[test]
    fn avoids_raw_long_requirement_for_english_prompt() {
        let requirement =
            "Create a stable React SQLite app for this requirement: customer follow up tracking";

        assert_eq!(suggest_software_name(requirement), "Customer Desk");
    }
}
