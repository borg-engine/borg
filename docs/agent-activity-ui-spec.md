# Agent Activity UI Redesign Spec

## Goal

Make the "agent is working" experience as polished as Claude's agent UI. The activity view is the proof layer that shows the agent doing real work — it needs to feel purposeful, trustworthy, and impressive.

## Current State (what we have)

- `LiveTerminal` component: rounded box with header ("Working...") and timeline of events
- `TimelineItem`: vertical line connector + icon + label + detail text
- `StreamEventBlock`: collapsible tool call/result blocks (used in completed task trace view)
- `AgentTimeline` in chat drawer: last 15 lines of activity shown inline

### Current problems

- Looks like a developer debug log, not a polished product
- Tool names shown raw ("Read", "Grep") rather than human-readable actions
- Tool inputs shown as truncated raw text, not formatted
- No smooth transitions — items just appear
- No visual hierarchy between "agent is thinking" vs "agent is doing something" vs "agent produced output"
- Timeline connector lines are basic
- No grouping of related actions (e.g. reading 3 files in a row)
- Result previews are plain monospace text dumps
- Chat drawer activity view is too cramped and disconnected from the main task view

## Target State (what Claude does)

### 1. Action Cards (replaces raw tool lines)

Each tool use is a clean, self-contained card:

```
┌─────────────────────────────────────────┐
│ 📄 Read file                            │
│ src/contracts/nda-template.md           │
│                                    ▼    │
└─────────────────────────────────────────┘
```

- **Icon**: contextual per tool type (file icon for Read, pencil for Edit, terminal for Bash, search for Grep, globe for WebFetch)
- **Label**: human-readable verb phrase, NOT the raw tool name
  - `Read` → "Read file"
  - `Edit` → "Edited file"
  - `Write` → "Created file"
  - `Grep` → "Searched for pattern"
  - `Glob` → "Found files matching"
  - `Bash` → "Ran command" (show the description field if available, not the raw command)
  - `WebFetch` → "Fetched page"
  - `WebSearch` → "Searched the web"
  - `Agent` → "Delegated to sub-agent"
- **Detail line**: the key parameter (file path, search pattern, command description) — one line, truncated
- **Expand chevron**: click to reveal full input/output
- **Collapsed by default** when not the most recent action
- Background: subtle, not white-on-dark — use `bg-[#1c1a17]` with `border-[#2a2520]`
- Border-radius: `rounded-xl` to match the rest of the dashboard

### 2. Expanded Card State

When expanded, show input and output in separate sections:

```
┌─────────────────────────────────────────┐
│ 📄 Read file                            │
│ src/contracts/nda-template.md           │
├─────────────────────────────────────────┤
│ ┃ # NDA Template                        │
│ ┃ This Non-Disclosure Agreement...      │
│ ┃ ...                                   │
│ ┃                          124 lines    │
└─────────────────────────────────────────┘
```

- Output shown in a scrollable area with `max-h-[300px]`
- Syntax highlighting for code/markdown if applicable
- Line count indicator at bottom right
- Smooth expand/collapse animation (`transition-[max-height]` or `grid-rows` trick)

### 3. Thinking / Reasoning Indicator

When the agent is between tool calls (generating text / thinking):

```
┌─────────────────────────────────────────┐
│ ✦ Thinking                              │
│ ░░░░░░░░░░░░░░░ (shimmer animation)     │
└─────────────────────────────────────────┘
```

- Subtle shimmer/pulse animation on the card
- No content shown (thinking is internal)
- Replaces current "Working..." ping dot which is too small
- Card should feel alive — subtle gradient shift or shimmer across the surface

### 4. Text Output Blocks

When the agent produces text (assistant messages between tool calls):

```
┌─────────────────────────────────────────┐
│ I've reviewed the contract and found    │
│ three key issues:                       │
│                                         │
│ 1. The non-compete clause extends...    │
│ 2. The indemnification section...       │
│ 3. There's no limitation of...          │
└─────────────────────────────────────────┘
```

- Render as markdown (already have `ChatMarkdown`)
- No icon/header — just the text in a clean card
- Slightly different background from tool cards to distinguish (`bg-transparent` or very subtle)
- Text streams in character by character if live (typewriter effect optional, but nice)

### 5. Phase Transitions

When the pipeline moves between phases:

```
──────────── Research ✓ ────────────
──────────── Drafting (active) ─────
```

- Full-width divider with phase name
- Completed phases get a checkmark and muted colors
- Active phase gets amber accent
- Shows progress through the pipeline without requiring the user to look at a separate phase strip

### 6. Progress Header

Replace the current minimal header with a richer status bar:

```
┌─────────────────────────────────────────┐
│ ● Working · Contract Review             │
│ Phase: Drafting (3/5) · 47 actions · 3m │
└─────────────────────────────────────────┘
```

- Pulsing dot (amber when active, green when done, red on error)
- Task/pipeline name
- Current phase + progress (e.g. "3/5" phases)
- Action count + elapsed time
- Sticky at top of the scroll area

### 7. Action Grouping

When the agent does multiple similar actions in a row (e.g. reads 5 files), collapse into a group:

```
┌─────────────────────────────────────────┐
│ 📄 Read 5 files                         │
│ src/contracts/nda.md, src/templates/... │
│                                    ▼    │
└─────────────────────────────────────────┘
```

- Group consecutive same-tool calls
- Show count + preview of targets
- Expand to see individual cards
- Reduces visual noise significantly

### 8. Error States

When a tool call fails:

```
┌─────────────────────────────────────────┐
│ ⚠ Command failed                       │
│ npm test                                │
│ Exit code 1                             │
├ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ┤
│ Error: Cannot find module 'xyz'         │
└─────────────────────────────────────────┘
```

- Red/orange accent border
- Error icon
- Auto-expanded to show the error
- Clear but not alarming — errors are normal in agent workflows

### 9. Final Output / Deliverable

When the agent produces its final output:

```
┌═════════════════════════════════════════┐
│ ✓ Contract Review Complete              │
│                                         │
│ [rendered markdown output]              │
│                                         │
│ ┌─────────┐ ┌──────────┐               │
│ │ Copy    │ │ Download │               │
│ └─────────┘ └──────────┘               │
└═════════════════════════════════════════┘
```

- Visually distinct from action cards (thicker border, subtle green/emerald accent)
- Full markdown rendering
- Copy + Download buttons
- This is what the user came for — make it prominent

### 10. Animations

- **Card entrance**: fade in + slight slide up (`animate-[fade-slide-up_0.2s_ease-out]`)
- **Expand/collapse**: smooth height transition (use `grid-rows-[0fr]` → `grid-rows-[1fr]` trick for CSS-only animation)
- **Thinking shimmer**: gradient sweep across the card surface
- **Phase transition**: horizontal line grows from center outward
- **Status dot**: `animate-pulse` for active, static for done
- **Auto-scroll**: smooth scroll to newest item, pause when user scrolls up (already implemented)

## Implementation Plan

### Phase 1: Action Card Component

Replace `TimelineItem` and `TimelineLineView` with a new `ActionCard` component:

- File: `dashboard/src/components/action-card.tsx`
- Props: `type`, `tool`, `label`, `detail`, `input`, `output`, `status`, `isLatest`
- Handles all event types (tool, text, system, result, phase_result, error)
- Human-readable label generation (tool name → verb phrase)
- Collapsible with smooth animation

### Phase 2: Update LiveTerminal

- Use `ActionCard` instead of `TimelineLineView`
- Add action grouping logic (consecutive same-tool calls)
- Richer header with phase info, action count, elapsed time
- Phase transition dividers

### Phase 3: Update Chat Drawer Activity

- Use same `ActionCard` component (compact variant)
- Show last few actions inline in chat
- Consistent look between task view and chat view

### Phase 4: Final Output Display

- Dedicated output card with markdown rendering
- Copy/download actions
- Visual prominence (border accent, spacing)

### Phase 5: Polish

- Animation tuning
- Loading states
- Error states
- Mobile responsive
- Performance (virtualize long lists if needed)

## Component Hierarchy

```
LiveTerminal
├── ProgressHeader (sticky)
├── ActionCard[] (scrollable)
│   ├── ActionCard (tool use — collapsible)
│   ├── ActionCard (text output)
│   ├── ActionCard (thinking indicator)
│   ├── PhaseTransition (divider)
│   └── ActionCard (final output — prominent)
└── ScrollAnchor
```

## Key Files to Modify

- `dashboard/src/components/live-terminal.tsx` — main rewrite
- `dashboard/src/components/borging.tsx` — replace TimelineItem with ActionCard
- `dashboard/src/components/chat-drawer.tsx` — use compact ActionCard variant
- `dashboard/src/components/task-detail.tsx` — StreamEventBlock → ActionCard
- `dashboard/src/lib/stream-utils.ts` — may need richer event parsing for grouping
- New: `dashboard/src/components/action-card.tsx`

## Human-Readable Label Map

| Tool | Label | Detail source |
|------|-------|---------------|
| Read | "Read file" | file_path |
| Write | "Created file" | file_path |
| Edit | "Edited file" | file_path |
| Bash | "Ran command" | description field, fallback to command preview |
| Grep | "Searched for" | pattern + path |
| Glob | "Found files" | pattern |
| WebFetch | "Fetched page" | URL |
| WebSearch | "Searched web" | query |
| Agent | "Sub-agent" | prompt preview |
| Task | "Created task" | task description |
| (thinking) | "Thinking..." | (no detail) |
| (text) | (none — just render text) | |
| (error) | "Error" or tool-specific | error message |
| (result) | "Complete" | output preview |
