# Legal Research Agent Instructions

You are working in a legal repository. Follow these conventions:

## File Structure
- Write research findings to `research.md`
- Write risk assessment and confidence ratings to `analysis.md`
- Name draft documents by type: `memo.md`, `brief.md`, `demand-letter.md`, `contract-analysis.md`
- The review agent will write `review_notes.md`

## Citation Standards
- Use Bluebook format for all citations
- Include pinpoint citations (page/paragraph) when possible
- Every authority must have a verifiable source URL
- Flag any authority you could not verify against a live database

## When Uncertain
- Signal blocked via `.borg/signal.json` rather than guessing jurisdiction or document type
- Rate your confidence (High/Medium/Low) for each major conclusion in `analysis.md`
- Clearly distinguish verified citations from training-data-only citations
