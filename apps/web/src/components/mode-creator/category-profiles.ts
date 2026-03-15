import type { IntegrationType, PhaseType } from "@/lib/types";

export interface CategoryProfile {
  phaseTypes: PhaseType[];
  behaviorFlags: string[];
  integrations: { value: IntegrationType; label: string }[];
  tools: string[];
  showDocker: boolean;
  showTestCmd: boolean;
  showComplianceButtons: boolean;
}

const ALL_TOOLS = ["Read", "Glob", "Grep", "Write", "Edit", "Bash", "WebSearch", "WebFetch"];
const DOC_TOOLS = ["Read", "Write", "WebSearch", "WebFetch"];

const CODE_PROFILE: CategoryProfile = {
  phaseTypes: ["setup", "agent", "validate", "rebase", "lint_fix", "human_review"],
  behaviorFlags: [
    "commits",
    "runs_tests",
    "use_docker",
    "include_task_context",
    "include_file_listing",
    "allow_no_changes",
    "fresh_session",
  ],
  integrations: [
    { value: "git_pr", label: "Git PR" },
    { value: "none", label: "None" },
  ],
  tools: ALL_TOOLS,
  showDocker: true,
  showTestCmd: true,
  showComplianceButtons: false,
};

const DOCUMENT_PROFILE: CategoryProfile = {
  phaseTypes: ["setup", "agent", "human_review", "compliance_check"],
  behaviorFlags: ["include_task_context", "allow_no_changes", "fresh_session"],
  integrations: [{ value: "none", label: "None" }],
  tools: DOC_TOOLS,
  showDocker: false,
  showTestCmd: false,
  showComplianceButtons: true,
};

const ALL_PROFILE: CategoryProfile = {
  phaseTypes: ["setup", "agent", "validate", "rebase", "lint_fix", "human_review", "compliance_check"],
  behaviorFlags: [
    "commits",
    "runs_tests",
    "use_docker",
    "include_task_context",
    "include_file_listing",
    "allow_no_changes",
    "fresh_session",
  ],
  integrations: [
    { value: "git_pr", label: "Git PR" },
    { value: "git_branch", label: "Git Branch" },
    { value: "none", label: "None" },
  ],
  tools: ALL_TOOLS,
  showDocker: true,
  showTestCmd: true,
  showComplianceButtons: true,
};

const GENERAL_PROFILE: CategoryProfile = {
  phaseTypes: ["setup", "agent", "human_review", "compliance_check"],
  behaviorFlags: ["include_task_context", "allow_no_changes", "fresh_session"],
  integrations: [
    { value: "git_pr", label: "Git PR" },
    { value: "none", label: "None" },
  ],
  tools: ALL_TOOLS,
  showDocker: false,
  showTestCmd: false,
  showComplianceButtons: true,
};

const KNOWLEDGE_PROFILE: CategoryProfile = {
  ...GENERAL_PROFILE,
  showComplianceButtons: false,
};

export function getProfile(category: string, showAll: boolean, dashboardMode?: string): CategoryProfile {
  if (dashboardMode === "swe") return showAll ? ALL_PROFILE : CODE_PROFILE;
  if (dashboardMode === "legal")
    return showAll ? { ...GENERAL_PROFILE, showComplianceButtons: true } : DOCUMENT_PROFILE;
  if (dashboardMode === "knowledge")
    return showAll ? { ...GENERAL_PROFILE, showComplianceButtons: false } : KNOWLEDGE_PROFILE;
  // Category-based detection
  const cat = (category || "").toLowerCase();
  if (cat.includes("engineering") || cat.includes("data")) return showAll ? ALL_PROFILE : CODE_PROFILE;
  if (cat.includes("professional") || cat.includes("legal") || cat.includes("people") || cat.includes("ops"))
    return showAll ? { ...GENERAL_PROFILE, showComplianceButtons: true } : DOCUMENT_PROFILE;
  return showAll ? GENERAL_PROFILE : KNOWLEDGE_PROFILE;
}
