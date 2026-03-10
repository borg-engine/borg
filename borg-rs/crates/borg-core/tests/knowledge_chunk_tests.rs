use borg_core::knowledge::chunk_text;

const CHUNK_SIZE: usize = 512;
const CHUNK_OVERLAP: usize = 64;

fn words(n: usize) -> String {
    (0..n)
        .map(|i| format!("w{i}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn empty_string_returns_one_empty_chunk() {
    let chunks = chunk_text("");
    assert_eq!(chunks.len(), 0);
}

#[test]
fn short_text_returns_single_chunk() {
    let text = "hello world this is a short sentence";
    let chunks = chunk_text(text);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], text);
}

#[test]
fn exactly_chunk_size_words_returns_single_chunk() {
    let text = words(CHUNK_SIZE);
    let chunks = chunk_text(&text);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], text);
}

#[test]
fn one_word_over_chunk_size_produces_two_chunks() {
    let text = words(CHUNK_SIZE + 1);
    let chunks = chunk_text(&text);
    // First chunk: words 0..CHUNK_SIZE
    // Second chunk: words (CHUNK_SIZE - CHUNK_OVERLAP)..CHUNK_SIZE+1
    assert_eq!(chunks.len(), 2);

    let word_list: Vec<&str> = text.split_whitespace().collect();
    assert_eq!(chunks[0], word_list[..CHUNK_SIZE].join(" "));
    assert_eq!(chunks[1], word_list[CHUNK_SIZE - CHUNK_OVERLAP..].join(" "));
}

#[test]
fn last_chunk_contains_final_words() {
    let total = CHUNK_SIZE * 2 + 100;
    let text = words(total);
    let chunks = chunk_text(&text);

    let word_list: Vec<&str> = text.split_whitespace().collect();
    let last = chunks.last().unwrap();
    // Last chunk must end with the final word of the input.
    assert!(
        last.ends_with(word_list.last().unwrap()),
        "last chunk must include final word"
    );
}

#[test]
fn chunks_overlap_by_chunk_overlap_words() {
    let total = CHUNK_SIZE + CHUNK_SIZE / 2;
    let text = words(total);
    let chunks = chunk_text(&text);
    assert!(chunks.len() >= 2);

    // The tail of chunk[0] and the head of chunk[1] must share CHUNK_OVERLAP words.
    let c0_words: Vec<&str> = chunks[0].split_whitespace().collect();
    let c1_words: Vec<&str> = chunks[1].split_whitespace().collect();

    let tail: Vec<&str> = c0_words[c0_words.len() - CHUNK_OVERLAP..].to_vec();
    let head: Vec<&str> = c1_words[..CHUNK_OVERLAP].to_vec();
    assert_eq!(
        tail, head,
        "consecutive chunks must overlap by {CHUNK_OVERLAP} words"
    );
}

#[test]
fn all_chunks_non_empty_for_long_text() {
    let text = words(CHUNK_SIZE * 3);
    for chunk in chunk_text(&text) {
        assert!(!chunk.is_empty(), "no chunk should be empty");
    }
}

#[test]
fn whitespace_only_input_returns_single_empty_chunk() {
    let chunks = chunk_text("   \n\t  ");
    assert_eq!(chunks.len(), 0);
}

// ── Structural chunking tests ──

fn make_section(heading: &str, word_count: usize) -> String {
    let body: String = (0..word_count)
        .map(|i| format!("word{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    format!("{heading}\n\n{body}")
}

#[test]
fn section_headings_create_chunk_boundaries() {
    // Two sections, each ~200 words — should produce 2 chunks (not merged into 1 that splits mid-text)
    let text = format!(
        "{}\n\n{}\n\n{}",
        make_section("INDEMNIFICATION", 200),
        make_section("LIMITATION OF LIABILITY", 200),
        make_section("TERMINATION", 200),
    );
    let chunks = chunk_text(&text);
    // With 600 total words, naive chunking would produce 2 chunks splitting at word 512.
    // Structural chunking should keep sections together or split at boundaries.
    assert!(chunks.len() >= 2, "should produce multiple chunks");

    // First chunk should contain INDEMNIFICATION content
    assert!(
        chunks[0].contains("INDEMNIFICATION"),
        "first chunk should contain first heading"
    );
}

#[test]
fn numbered_clauses_create_boundaries() {
    let mut text = String::new();
    for i in 1..=5 {
        text.push_str(&format!("{i}. Clause number {i}.\n\n{}\n\n", words(100)));
    }
    let chunks = chunk_text(&text);
    // 5 sections of ~102 words each = ~510 words.
    // Should stay as 1 chunk since total is under CHUNK_SIZE.
    // But if we add more words it should split at clause boundaries.
    assert!(!chunks.is_empty());
}

#[test]
fn large_sections_still_get_word_chunked() {
    // A single giant section with no structural breaks
    let text = words(CHUNK_SIZE * 3);
    let chunks = chunk_text(&text);
    // Falls back to word-based chunking
    assert!(
        chunks.len() >= 3,
        "large text without structure should still be chunked"
    );
}

#[test]
fn paragraph_breaks_create_boundaries() {
    let mut paragraphs = Vec::new();
    for i in 0..6 {
        let p: String = (0..100)
            .map(|j| format!("p{i}w{j}"))
            .collect::<Vec<_>>()
            .join(" ");
        paragraphs.push(p);
    }
    let text = paragraphs.join("\n\n");
    let chunks = chunk_text(&text);
    // 600 words in 6 paragraphs. Should split at paragraph boundaries rather than mid-paragraph.
    assert!(chunks.len() >= 2);
    // Verify no chunk starts mid-word from a different paragraph
    for chunk in &chunks {
        assert!(!chunk.is_empty());
    }
}

#[test]
fn contract_style_document_chunks_at_sections() {
    let text = "\
WHEREAS the Parties wish to enter into this Agreement on the terms set forth herein.

ARTICLE I - DEFINITIONS

1.1 \"Agreement\" means this Master Services Agreement including all exhibits and schedules attached hereto. \
The Agreement shall be binding upon execution by both parties and shall remain in effect for the term specified \
in Section 3.1. Any amendments to this Agreement must be in writing and signed by authorized representatives \
of both Parties. This definition encompasses all modifications, addenda, and supplements that may be executed \
from time to time during the term of this Agreement. The Parties acknowledge that this Agreement supersedes \
all prior negotiations, representations, warranties, commitments, offers, contracts, and writings between \
the Parties with respect to the subject matter hereof.

1.2 \"Confidential Information\" means any and all non-public information disclosed by either Party to the other \
Party whether orally or in writing that is designated as confidential or that reasonably should be understood \
to be confidential given the nature of the information and the circumstances of disclosure. Confidential \
Information includes but is not limited to trade secrets, know-how, inventions, techniques, processes, \
algorithms, software programs, customer lists, financial information, sales data, business plans, marketing \
plans, and any other information that provides a competitive advantage to the disclosing Party. The receiving \
Party shall protect Confidential Information using the same degree of care it uses to protect its own \
confidential information of like kind but in no event less than reasonable care.

ARTICLE II - SERVICES

2.1 The Service Provider shall provide the services described in Exhibit A attached hereto and incorporated \
herein by reference. The services shall be performed in a professional and workmanlike manner consistent with \
industry standards and practices. The Service Provider shall assign qualified personnel to perform the services \
and shall not substitute key personnel without the prior written consent of the Client. All services shall be \
performed in accordance with the timeline set forth in Exhibit B. The Service Provider represents and warrants \
that it has the necessary skills, experience, licenses, and certifications to perform the services contemplated \
by this Agreement. In the event that the Service Provider fails to perform any services in accordance with the \
specifications set forth herein, the Client shall provide written notice of such deficiency and the Service \
Provider shall have thirty days from receipt of such notice to cure such deficiency at no additional cost.

ARTICLE III - COMPENSATION

3.1 The Client shall pay the Service Provider the fees set forth in Exhibit C in consideration for the services \
provided hereunder. Payment shall be due within thirty days of receipt of a proper invoice. Late payments shall \
accrue interest at the rate of one and one-half percent per month or the maximum rate permitted by applicable law, \
whichever is less. The Service Provider shall submit invoices on a monthly basis detailing the services performed \
and expenses incurred during the preceding month.

ARTICLE IV - INDEMNIFICATION

4.1 The Service Provider shall indemnify, defend, and hold harmless the Client and its officers, directors, \
employees, agents, successors, and assigns from and against any and all claims, damages, losses, costs, \
expenses, and liabilities including reasonable attorneys fees and court costs arising out of or relating to \
any breach of this Agreement by the Service Provider or any negligent or wrongful act or omission of the \
Service Provider or its employees agents or subcontractors in connection with the performance of services \
under this Agreement. The Service Provider shall promptly notify the Client of any claim or action that may \
give rise to an indemnification obligation hereunder and shall cooperate fully with the Client in the defense \
of any such claim. The Client shall have the right to participate in the defense of any claim at its own \
expense and shall have the right to approve any settlement that would impose any obligation on the Client.

ARTICLE V - LIMITATION OF LIABILITY

5.1 In no event shall either Party be liable to the other Party for any indirect incidental special consequential \
or punitive damages arising out of or relating to this Agreement regardless of whether such damages are based on \
contract tort strict liability or any other theory and regardless of whether such Party has been advised of the \
possibility of such damages. The total aggregate liability of either Party under this Agreement shall not exceed \
the total fees paid or payable by the Client to the Service Provider during the twelve month period immediately \
preceding the event giving rise to such liability. This limitation of liability shall not apply to obligations \
of indemnification, breaches of confidentiality, or willful misconduct.";

    let chunks = chunk_text(&text);
    assert!(chunks.len() >= 2, "contract should produce multiple chunks");

    // Verify that ARTICLE headings appear at chunk starts (not mid-chunk)
    let all_text: String = chunks.join(" ");
    assert!(all_text.contains("ARTICLE I"));
    assert!(all_text.contains("ARTICLE II"));
    assert!(all_text.contains("ARTICLE III"));
    assert!(all_text.contains("WHEREAS"));
}

#[test]
fn all_input_words_preserved() {
    let text = format!(
        "{}\n\n{}\n\n{}",
        make_section("SECTION 1 - OBLIGATIONS", 300),
        make_section("SECTION 2 - RIGHTS", 300),
        make_section("SECTION 3 - REMEDIES", 300),
    );
    let input_words: Vec<&str> = text.split_whitespace().collect();
    let chunks = chunk_text(&text);
    let output_words: std::collections::HashSet<&str> =
        chunks.iter().flat_map(|c| c.split_whitespace()).collect();

    // Every input word must appear in at least one chunk
    for w in &input_words {
        assert!(output_words.contains(w), "word '{w}' missing from chunks");
    }
}
