# UI Simplification Plan

## Architecture Summary

Merge projects and tasks into a single "Matters" primitive. Remove tasks as a separate nav concept. Merge agent activity into chat. Add "Memory" as a pinned top-level item for global knowledge.

### Nav Structure
- **Memory** (pinned at top of matters list, global cross-project knowledge)
- **Matters** (list with status indicators — the primary and almost only view)
- **Pipelines** (SWE only)
- **Settings**

### Matter Detail View
- **Documents** (left panel or tab — project-scoped files)
- **Chat** (main area — conversation + inline agent activity with collapsible action cards)

---

## Task List

### 1. Data Model: Merge Project + Task

**What:** Make a "matter" the single primitive. A matter has documents, chat history, and work items. Work items are what tasks currently are — an agent execution running through a pipeline. They belong to a matter, not as a separate top-level entity.

**Details:**
- Tasks already have a `project_id` foreign key — this is the link
- Remove the concept of standalone tasks (tasks without a project)
- When a user asks the agent to do work in a matter's chat, it creates a work item (task) under that matter automatically
- Work items track pipeline phase, status, agent stream, outputs — same as current tasks
- A matter can have multiple concurrent or sequential work items

**Files:** `borg-rs/crates/borg-core/src/types.rs`, `borg-rs/crates/borg-core/src/db.rs`, `borg-rs/crates/borg-core/src/pipeline.rs`

---

### 2. Backend: Wire Work Items Into Chat

**What:** When a work item is created from chat, its agent activity stream should be broadcastable to the chat SSE channel so the frontend can render it inline.

**Details:**
- `/api/chat/events` SSE already receives `chat_stream` events with NDJSON tool use data
- Ensure work item creation from chat tags the work item with the chat thread
- Forward the work item's `stream_tx` events to the chat event channel
- When work item completes, send the final output as a chat message (assistant role)
- Include work item metadata (pipeline phase, status) in stream events

**Files:** `borg-rs/crates/borg-server/src/routes.rs`, `borg-rs/crates/borg-core/src/stream.rs`, `borg-rs/crates/borg-core/src/pipeline.rs`

---

### 3. Frontend: Remove Tasks Nav Item

**What:** Remove "Tasks" from the sidebar nav. Remove the standalone tasks list and task detail views from the main layout.

**Details:**
- Remove `tasks` from `ALL_NAV_ITEMS` and `View` type
- Remove `TaskList` and `TaskDetail` imports/rendering from `App.tsx`
- Don't delete the components yet — parts will be reused in the matter detail view
- Update `header.tsx` View type

**Files:** `dashboard/src/App.tsx`, `dashboard/src/components/header.tsx`

---

### 4. Frontend: Matter List with Status Indicators

**What:** Update the projects/matters sidebar list to show work item status at a glance.

**Details:**
- Amber pulsing dot = agent actively working
- Orange circle = needs human review (human_review phase)
- Green check = all work complete
- No indicator = idle
- Fetch active work item status per project from API (extend `/api/projects` or add `/api/projects/:id/status`)
- Sort: active work at top, then needs review, then idle

**Files:** `dashboard/src/components/projects-panel.tsx`, `borg-rs/crates/borg-server/src/routes.rs`

---

### 5. Frontend: Memory — Pinned Global Knowledge Item

**What:** Add a "Memory" item pinned at the top of the matters list. Uses the same UI as a matter but scoped globally.

**Details:**
- Pinned above the matters list with a distinct icon (brain or similar)
- Clicking opens the same detail view: documents panel + chat
- Documents uploaded here are global knowledge (available to all matters)
- Chat here is the global thread (`web:dashboard`)
- No pipeline/work items — just knowledge + chat
- Backend: could be a special project with `is_global: true` flag, or use existing global knowledge endpoints

**Files:** `dashboard/src/components/projects-panel.tsx`, `dashboard/src/lib/api.ts`, possibly `borg-rs/crates/borg-core/src/db.rs`

---

### 6. Frontend: Action Card Component (from agent-activity-ui-spec.md)

**What:** New `ActionCard` component that renders agent tool use beautifully, replacing `TimelineItem`, `TimelineLineView`, and `StreamEventBlock`.

**Details:**
- Three levels of detail: summary (one line) → expanded (input/output) → raw (full NDJSON)
- Human-readable labels ("Read file" not "Read")
- Contextual icons per tool type
- Smooth expand/collapse animation
- Compact variant for inline chat use
- See `docs/agent-activity-ui-spec.md` for full spec

**Files:** New `dashboard/src/components/action-card.tsx`

---

### 7. Frontend: Inline Agent Activity in Chat

**What:** Render agent work inline in the chat conversation using ActionCards instead of in a separate LiveTerminal.

**Details:**
- When the agent is working on a chat message, show action cards inline after the user's message
- Action cards are collapsed by default, expandable for inspection
- Consecutive tool calls of the same type get grouped ("Read 5 files")
- Thinking indicator between tool calls (shimmer card)
- Final output rendered as a rich assistant message with markdown
- Auto-scroll follows the activity, pauses when user scrolls up
- Replaces the current `AgentTimeline` in chat drawer
- Replaces `LiveTerminal` for chat-originated work

**Files:** `dashboard/src/components/chat-drawer.tsx`, `dashboard/src/lib/use-chat-events.ts`, `dashboard/src/lib/stream-utils.ts`

---

### 8. Frontend: Matter Detail View Restructure

**What:** Redesign the project detail view to be Documents + Chat as the two main areas.

**Details:**
- Left panel: document list with upload, similar to current knowledge panel but project-scoped
- Right/main area: chat with inline agent activity
- No separate tabs for "overview", "tasks", "activity" — chat IS the activity view
- Active work items shown as in-progress action card sequences in the chat
- Completed work items shown as collapsed summaries in chat history
- Header shows matter name, client info, status indicator

**Files:** `dashboard/src/components/project-detail.tsx`, `dashboard/src/components/projects-panel.tsx`

---

### 9. Frontend: Work Item Controls in Chat

**What:** Add controls for managing work items from within the chat context.

**Details:**
- When a work item is in `human_review` phase, show approve/reject/revise buttons inline
- When a work item fails, show retry button inline
- Pipeline phase indicator shown subtly above the action card group
- Cancel button for in-progress work
- These replace the current task detail header controls

**Files:** `dashboard/src/components/chat-drawer.tsx` or new component

---

### 10. Backend: API Cleanup

**What:** Ensure APIs support the merged model cleanly.

**Details:**
- `/api/projects` returns work item status summary per project
- `/api/projects/:id/work` returns work items for a project
- `/api/chat` POST can trigger work item creation (already works)
- Work item stream events include project context
- Global knowledge endpoints mapped to Memory concept
- Deprecate standalone task creation outside of project context (or auto-create a project)

**Files:** `borg-rs/crates/borg-server/src/routes.rs`, `borg-rs/crates/borg-core/src/db.rs`

---

### 11. Frontend: Simplify Sidebar Nav

**What:** Final nav cleanup — only show what's needed per mode.

**Details:**
- Legal mode: Memory + Matters + Settings (3 items)
- SWE mode: Memory + Matters + Pipelines + Auto Tasks + Settings (5 items)
- Remove: Tasks, Proposals, Queue, Logs from default nav
- Logs accessible from Settings or as a debug toggle
- Queue/Proposals could become filters on the matters list rather than separate views

**Files:** `dashboard/src/App.tsx`

---

### 12. Migration: Existing Data

**What:** Ensure existing tasks and projects map cleanly to the new model.

**Details:**
- Tasks with a `project_id` already belong to a matter — no migration needed
- Orphan tasks (no project) either get auto-assigned to a default project or shown in a legacy view
- Chat history preserved as-is
- No destructive database changes — additive only

---

## Execution Order

1. **Task 6** — ActionCard component (foundation, no dependencies)
2. **Task 1** — Data model alignment (backend foundation)
3. **Task 2** — Wire work items into chat (backend)
4. **Task 10** — API cleanup (backend)
5. **Task 3** — Remove Tasks nav (frontend, quick)
6. **Task 5** — Memory item (frontend)
7. **Task 4** — Matter list status indicators (frontend)
8. **Task 7** — Inline agent activity in chat (frontend, biggest piece)
9. **Task 8** — Matter detail restructure (frontend)
10. **Task 9** — Work item controls in chat (frontend)
11. **Task 11** — Simplify nav (frontend, quick)
12. **Task 12** — Migration verification
