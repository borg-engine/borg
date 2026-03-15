use std::collections::HashSet;

use crate::{
    db::ProjectFileStats,
    types::{PhaseConfig, Task},
};

#[derive(Debug, Clone, Default)]
pub(crate) struct LegalRetrievalTrace {
    pub(crate) enforced: bool,
    pub(crate) passed: bool,
    pub(crate) trigger_source: String,
    pub(crate) inventory_calls: usize,
    pub(crate) category_calls: usize,
    pub(crate) search_calls: usize,
    pub(crate) coverage_calls: usize,
    pub(crate) full_document_reads: usize,
    pub(crate) search_queries: Vec<String>,
    pub(crate) distinct_search_queries: Vec<String>,
    pub(crate) coverage_queries: Vec<String>,
    pub(crate) mcp_servers: Vec<serde_json::Value>,
    pub(crate) missing_steps: Vec<String>,
}

pub(crate) fn is_negative_sign_recommendation(normalized: &str) -> bool {
    contains_any(
        normalized,
        &[
            "do not sign",
            "cannot sign",
            "should not sign",
            "must not sign",
            "not ready to sign",
            "not supportable",
            "signing is not supportable",
            "sign is not supportable",
            "not recommend signing",
            "recommend against signing",
            "sign recommendation not finalised",
            "sign recommendation not finalized",
            "recommendation not finalised",
            "recommendation not finalized",
            "sign recommendation suspended",
            "recommendation suspended",
            "blocked — sign",
            "blocked - sign",
            "sign recommendation: blocked",
            "sign recommendation blocked",
            "not proceed to sign",
            "do not close",
            "cannot close",
            "should not close",
            "must not close",
            "not ready to close",
            "closing is not supportable",
            "close is not supportable",
            "not recommend closing",
            "recommend against closing",
            "close recommendation not finalised",
            "close recommendation not finalized",
            "close recommendation suspended",
            "close recommendation: blocked",
            "close recommendation blocked",
            "not proceed to close",
        ],
    )
}

pub(crate) fn detect_benchmark_clarification_escape(text: &str) -> Option<String> {
    let normalized = text.to_ascii_lowercase();
    if is_enforcement_status_warranty_safe_harbor(&normalized) {
        return None;
    }
    if is_non_dispositive_tail_diligence_safe_harbor(&normalized) {
        return None;
    }
    if is_negative_sign_recommendation(&normalized) {
        return None;
    }
    let has_sign_or_close_position = contains_any(
        &normalized,
        &[
            "sign position",
            "sign recommendation",
            "sign-and-fix route",
            "sign and fix route",
            "sign-and-close route",
            "sign and close route",
            "recommended position: sign",
            "recommended sign-off position",
            "recommended sign off position",
            "signing can proceed",
            "sign can proceed",
            "can sign",
            "sign on ",
            "sign subject to",
            "signing is supportable",
            "sign is supportable",
            "sign remains supportable",
            "sign recommendation",
            "signing recommendation",
            "recommended sign position",
            "supportable with",
            "recommend sign",
            "recommend signing",
            "proceed to sign",
            "ready to sign",
            "no absolute sign-blockers",
            "no absolute sign blockers",
            "no sign-blockers",
            "no sign blockers",
            "no hard blockers to signing",
            "no hard blocker to signing",
            "closing position",
            "close recommendation",
            "closing recommendation",
            "recommended close position",
            "can close",
            "close is supportable",
            "closing is supportable",
            "recommend close",
            "proceed to close",
        ],
    );
    if !has_sign_or_close_position {
        return None;
    }

    let has_pre_sign_timing = contains_any(
        &normalized,
        &[
            "pre-sign",
            "pre sign",
            "before sign",
            "before signing",
            "before execution",
            "before execute",
            "prior to sign",
            "prior to signing",
            "prior to execution",
            "subject to",
            "pre-close",
            "pre close",
            "before close",
            "before closing",
            "prior to close",
            "prior to closing",
        ],
    );
    let has_unresolved_fact_language = contains_any(
        &normalized,
        &[
            "confirm",
            "confirmation",
            "verify",
            "verification",
            "check before",
            "must be checked",
            "must be confirmed",
            "must confirm",
            "must be resolved",
            "not confirmed",
            "not yet confirmed",
            "seller confirmation",
            "open factual question",
            "open factual questions",
            "management-presentation answers",
            "management presentation answers",
            "management presentation",
            "human review",
            "flagged for human review",
            "conditioned on",
            "open question",
            "clarification",
            "unresolved",
            "unknown",
            "pending",
        ],
    );
    if !has_unresolved_fact_language {
        return None;
    }

    let has_independence_signal = contains_any(
        &normalized,
        &[
            "recommendation is stable",
            "sign recommendation is stable",
            "close recommendation is stable",
            "recommendation is the same",
            "sign recommendation is the same",
            "close recommendation is the same",
            "same in all scenarios",
            "does not depend on",
            "do not depend on",
            "does not depend on resolving",
            "does not depend on receiving",
            "not dependent on resolving",
            "not dependent on",
            "not depend on first receiving",
            "whichever way each resolves",
            "whichever way that fact resolves",
            "whichever way this resolves",
            "whichever way",
            "regardless of",
            "irrespective of",
            "unconditional on",
            "does not block signing",
            "does not block sign",
            "do not block signing",
            "not a sign blocker",
            "not a sign-blocker",
        ],
    );

    if has_independence_signal {
        return None;
    }

    if has_pre_sign_timing {
        return Some(first_sentence_like_excerpt(text, 220));
    }

    let has_non_dispositive_override = contains_any(
        &normalized,
        &[
            "not a pre-sign condition",
            "not a pre sign condition",
            "not a pre-sign prerequisite",
            "not a pre sign prerequisite",
            "subject to the following",
            "subject to pre-sign",
            "subject to pre sign",
            "pre-sign requirements",
            "pre sign requirements",
            "not a closing condition",
            "not a blocker",
            "risk-reduction step, not a pre-sign prerequisite",
            "risk reduction step, not a pre-sign prerequisite",
            "post-close remediation",
            "post close remediation",
            "pre-close remediation",
            "pre close remediation",
            "open question",
            "questions for seller",
        ],
    );
    if !has_non_dispositive_override {
        return None;
    }

    Some(first_sentence_like_excerpt(text, 260))
}

fn is_enforcement_status_warranty_safe_harbor(normalized: &str) -> bool {
    let has_enforcement_notice_context = contains_any(
        normalized,
        &[
            "step-in notice",
            "step in notice",
            "suspension notice",
            "breach notice",
            "enforcement communication",
            "enforcement-status",
            "enforcement status",
        ],
    );
    if !has_enforcement_notice_context {
        return false;
    }

    let has_management_only_knowledge_limit = contains_any(
        normalized,
        &[
            "management is not aware",
            "management has confirmed",
            "management's current representation",
            "current representation",
            "no independent enforcement-status confirmation",
            "no independent enforcement status confirmation",
            "no independent confirmation",
        ],
    );
    if !has_management_only_knowledge_limit {
        return false;
    }

    contains_any(
        normalized,
        &[
            "spa warranty",
            "seller warranty",
            "warranty on this point",
            "must not be qualified by a knowledge limitation",
            "must not be qualified by a knowledge qualifier",
            "without a knowledge limitation",
            "without a knowledge qualifier",
            "unqualified warranty",
            "specific indemnity",
        ],
    )
}

fn is_non_dispositive_tail_diligence_safe_harbor(normalized: &str) -> bool {
    let has_tail_or_price_issue = contains_any(
        normalized,
        &[
            "mariner",
            "beacon retail finance",
            "other customers",
            "price mechanics",
            "completion accounts",
            "locked-box",
            "locked box",
            "tail customers",
            "contracts not reviewed",
        ],
    );
    if !has_tail_or_price_issue {
        return false;
    }

    let explicitly_non_dispositive = contains_any(
        normalized,
        &[
            "sign recommendation does not depend on",
            "sign decision does not depend on",
            "sign holds regardless",
            "holds regardless of what",
            "managed as a cp or post-close item",
            "managed as a cp or post close item",
            "price adjustment",
            "informs price mechanics",
            "not a sign blocker",
        ],
    );
    if !explicitly_non_dispositive {
        return false;
    }

    !contains_any(
        normalized,
        &[
            "titanbank procurement/legal",
            "titanbank procurement or legal",
            "approval notice",
            "genassist",
            "boroughcare authority",
            "northcounty",
            "step-in notice",
            "suspension notice",
            "breach notice",
            "authority approval",
            "schedule 5.4",
            "schedule 5.5",
        ],
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn first_sentence_like_excerpt(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= max_chars {
        return compact;
    }
    let end = compact.floor_char_boundary(max_chars);
    format!("{}...", &compact[..end])
}

pub(crate) fn failure_repeat_block_threshold(error: &str) -> u32 {
    if error.starts_with("Benchmark clarification guard failed.") {
        2
    } else {
        3
    }
}

pub(crate) fn legal_retrieval_protocol_trigger(
    task: &Task,
    phase: &PhaseConfig,
    stats: &ProjectFileStats,
) -> Option<&'static str> {
    if phase.name != "implement" {
        return None;
    }
    if !matches!(task.mode.as_str(), "lawborg" | "legal") {
        return None;
    }
    if task.project_id <= 0 || stats.text_files <= 0 {
        return None;
    }
    if task.requires_exhaustive_corpus_review {
        return Some("explicit");
    }

    let task_type = task.task_type.trim().to_ascii_lowercase();
    if matches!(
        task_type.as_str(),
        "contract_analysis"
            | "contract_review"
            | "nda_triage"
            | "nda"
            | "compliance"
            | "regulatory_analysis"
            | "vendor_check"
            | "clause_review"
    ) {
        return Some("heuristic_task_type");
    }

    let haystack =
        format!("{} {} {}", task.title, task.description, task.task_type).to_ascii_lowercase();
    [
        "review the legal documents",
        "review all documents",
        "review the documents in this repository",
        "reviewing project documents",
        "uploaded documents",
        "project documents",
        "document corpus",
        "clause extraction",
        "contract review",
        "contract analysis",
        "compliance audit",
        "due diligence",
        "all agreements",
        "across all documents",
    ]
    .iter()
    .find(|needle| haystack.contains(**needle))
    .map(|_| "heuristic_description")
}

pub(crate) fn prior_retrieval_protocol_passed_from_structured_data(
    value: &serde_json::Value,
) -> Option<bool> {
    value.get("retrieval_protocol")?.get("passed")?.as_bool()
}

pub(crate) fn should_reuse_prior_retrieval_pass(
    task: &Task,
    prior_passed: bool,
    current_passed: bool,
) -> bool {
    if current_passed || !should_offer_retrieval_reuse_guidance(task, prior_passed) {
        return false;
    }
    true
}

pub(crate) fn should_offer_retrieval_reuse_guidance(task: &Task, prior_passed: bool) -> bool {
    if !prior_passed || task.attempt <= 0 {
        return false;
    }
    true
}

fn latest_retry_error(last_error: &str) -> &str {
    last_error
        .trim()
        .split("\nLatest error:\n")
        .nth(1)
        .map(str::trim)
        .unwrap_or_else(|| last_error.trim())
}

fn _is_clarification_resume_error(error: &str) -> bool {
    let clarification_retry =
        error.starts_with("Material fact missing") && error.contains("\n\nQuestion:");
    let clarification_guard_retry = error.starts_with("Benchmark clarification guard failed.");
    clarification_retry || clarification_guard_retry
}

fn benchmark_clarification_question_from_structured_data(
    value: &serde_json::Value,
) -> Option<String> {
    let state = value.get("benchmark_state")?;
    if state.get("status")?.as_str()? != "blocked_for_clarification" {
        return None;
    }
    let question = state.get("question")?.as_str()?.trim();
    if question.is_empty() {
        None
    } else {
        Some(question.to_string())
    }
}

pub(crate) fn clarification_resume_question(
    prior_report: Option<&serde_json::Value>,
    last_error: &str,
) -> Option<String> {
    if let Some(question) =
        prior_report.and_then(benchmark_clarification_question_from_structured_data)
    {
        return Some(question);
    }
    let error = latest_retry_error(last_error);
    let question = error.split("\n\nQuestion:").nth(1)?.trim();
    if question.is_empty() {
        None
    } else {
        Some(question.to_string())
    }
}

pub(crate) fn inspect_legal_retrieval_trace(raw_stream: &str) -> LegalRetrievalTrace {
    let mut trace = LegalRetrievalTrace::default();
    let mut distinct_queries = HashSet::new();

    for line in raw_stream.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };

        if value.get("type").and_then(|v| v.as_str()) == Some("system")
            && value.get("subtype").and_then(|v| v.as_str()) == Some("init")
        {
            if let Some(servers) = value.get("mcp_servers").and_then(|v| v.as_array()) {
                trace.mcp_servers = servers.clone();
            }
            continue;
        }

        let event_type = value.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if event_type == "tool_use" {
            let name = value.get("tool").and_then(|v| v.as_str()).unwrap_or_default();
            let input = value
                .get("input")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            process_retrieval_tool_call(
                name,
                &input,
                &mut trace,
                &mut distinct_queries,
            );
            continue;
        }

        if event_type != "assistant" {
            continue;
        }
        let Some(blocks) = value
            .get("message")
            .and_then(|v| v.get("content"))
            .and_then(|v| v.as_array())
        else {
            continue;
        };

        for block in blocks {
            if block.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                continue;
            }
            let name = block
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            let input = block
                .get("input")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));

            process_retrieval_tool_call(
                name,
                &input,
                &mut trace,
                &mut distinct_queries,
            );
        }
    }

    trace
}

fn process_retrieval_tool_call(
    name: &str,
    input: &serde_json::Value,
    trace: &mut LegalRetrievalTrace,
    distinct_queries: &mut HashSet<String>,
) {
    match normalize_tool_name(name) {
        "list_documents" => trace.inventory_calls += 1,
        "get_document_categories" => trace.category_calls += 1,
        "search_documents" => {
            trace.search_calls += 1;
            if let Some(query) = extract_trace_query(input, &["query", "q"]) {
                let normalized = normalize_trace_query(&query);
                if distinct_queries.insert(normalized.clone()) {
                    trace.distinct_search_queries.push(query.clone());
                }
                trace.search_queries.push(query);
            }
        }
        "check_coverage" => {
            trace.coverage_calls += 1;
            if let Some(query) = extract_trace_query(input, &["query", "q"]) {
                trace.coverage_queries.push(query);
            }
        }
        "read_document" => trace.full_document_reads += 1,
        "Read"
            if input
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.contains("/project_files/") || s.starts_with("project_files/"))
                .unwrap_or(false) =>
        {
            trace.full_document_reads += 1;
        }
        "WebFetch" => classify_borg_webfetch(input, trace, distinct_queries),
        _ => {}
    }
}

fn normalize_tool_name(name: &str) -> &str {
    name.rsplit("__")
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or(name)
}

fn classify_borg_webfetch(
    input: &serde_json::Value,
    trace: &mut LegalRetrievalTrace,
    distinct_queries: &mut HashSet<String>,
) {
    let Some(url) = input.get("url").and_then(|v| v.as_str()) else {
        return;
    };
    if !url.contains("/api/borgsearch/") {
        return;
    }

    if url.contains("/api/borgsearch/query?") {
        trace.search_calls += 1;
        if let Some(query) = extract_query_param(url, "q") {
            let normalized = normalize_trace_query(&query);
            if distinct_queries.insert(normalized) {
                trace.distinct_search_queries.push(query.clone());
            }
            trace.search_queries.push(query);
        }
        return;
    }
    if url.contains("/api/borgsearch/files?") {
        trace.inventory_calls += 1;
        return;
    }
    if url.contains("/api/borgsearch/coverage?") {
        trace.coverage_calls += 1;
        if let Some(query) = extract_query_param(url, "q") {
            trace.coverage_queries.push(query);
        }
        return;
    }
    if url.contains("/api/borgsearch/facets?") {
        trace.category_calls += 1;
        return;
    }
    if url.contains("/api/borgsearch/file/") {
        trace.full_document_reads += 1;
    }
}

fn extract_trace_query(input: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| input.get(key))
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn extract_query_param(url: &str, key: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        if k == key {
            let decoded = v.replace('+', " ");
            let trimmed = decoded.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn normalize_trace_query(query: &str) -> String {
    query
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}
