# Legal Repository

This repository is managed by Borg's legal research agent (lawborg).

## Output Conventions

- `research.md` — Research memo with citations, methodology, and source URLs
- `analysis.md` — Risk assessment, confidence ratings, and limitations
- `review_notes.md` — Independent review checklist results
- Draft documents are named by type (e.g., `memo.md`, `brief.md`, `demand-letter.md`)

## Citation Format

All citations follow Bluebook format:
- US cases: *Smith v. Jones*, 550 U.S. 124, 130 (2007)
- US statutes: 42 U.S.C. § 1983 (2018)
- US regulations: 17 C.F.R. § 240.10b-5 (2023)
- UK cases: [2021] UKSC 35
- EU cases: Case C-131/12, ECLI:EU:C:2014:317
- Canadian cases: *R v. Oakes*, [1986] 1 SCR 103

Every citation must include a source URL or database identifier.

## Signals

If a task is missing critical context (jurisdiction, parties, document type), the agent writes
`{"status":"blocked","reason":"..."}` to `.borg/signal.json` instead of guessing.

## Confidentiality

Documents involving client matters include the header:
PRIVILEGED AND CONFIDENTIAL — ATTORNEY WORK PRODUCT
