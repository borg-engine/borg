use borg_core::types::{IntegrationType, PhaseConfig, PipelineMode, SeedConfig, SeedOutputType};

use crate::{agent_phase, setup_phase};

pub fn health_mode() -> PipelineMode {
    PipelineMode {
        name: "healthborg".into(),
        label: "Healthcare".into(),
        category: "Healthcare".into(),
        initial_status: "backlog".into(),
        uses_git_worktrees: true,
        uses_docker: true,
        uses_test_cmd: false,
        integration: IntegrationType::GitBranch,
        default_max_attempts: 3,
        phases: vec![
            setup_phase("implement"),
            PhaseConfig {
                include_task_context: true,
                include_file_listing: true,
                error_instruction: HEALTH_IMPLEMENT_RETRY.into(),
                commits: true,
                commit_message: "health: analysis from healthborg agent".into(),
                ..agent_phase(
                    "implement",
                    "Implement",
                    HEALTH_IMPLEMENT_SYSTEM,
                    HEALTH_IMPLEMENT_INSTRUCTION,
                    "Read,Glob,Grep,Write,Edit",
                    "done",
                )
            },
        ],
        seed_modes: vec![
            SeedConfig {
                name: "review".into(),
                label: "Document Review".into(),
                output_type: SeedOutputType::Task,
                prompt: HEALTH_SEED_REVIEW.into(),
                allowed_tools: "Read,Glob,Grep".into(),
                target_primary_repo: false,
            },
        ],
    }
}

const HEALTH_IMPLEMENT_SYSTEM: &str = "\
You are an autonomous healthcare document analysis agent. You review, \
summarize, and organize medical and health-related documents with strict \
attention to accuracy. Never fabricate medical information.";

const HEALTH_IMPLEMENT_INSTRUCTION: &str = "\
Handle this healthcare document task:
1. Read and understand all relevant documents
2. Produce the requested analysis, summary, or organization
3. Cite sources for all factual claims
4. Flag any ambiguities or missing information

Write your output as a clear markdown document.\n\
If the task is unclear, write {\"status\":\"blocked\",\"reason\":\"...\"} to .borg/signal.json.";

const HEALTH_IMPLEMENT_RETRY: &str =
    "\n\nPrevious attempt failed. Error:\n```\n{ERROR}\n```\nFix the issue.";

const HEALTH_SEED_REVIEW: &str = "Survey the health documents in this repository. Identify \
\ndocuments that need review, summarization, or organization. \
\nCreate a task for each actionable item.";
