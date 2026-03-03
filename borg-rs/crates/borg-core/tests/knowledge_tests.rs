use borg_core::knowledge::{bytes_to_embedding, cosine_similarity, embedding_to_bytes, hash_chunk};

// ── cosine_similarity ────────────────────────────────────────────────────────

#[test]
fn identical_vectors_return_one() {
    let v = vec![1.0f32, 2.0, 3.0];
    let result = cosine_similarity(&v, &v);
    assert!((result - 1.0).abs() < 1e-6, "identical vectors should return 1.0, got {result}");
}

#[test]
fn orthogonal_vectors_return_zero() {
    let a = vec![1.0f32, 0.0];
    let b = vec![0.0f32, 1.0];
    let result = cosine_similarity(&a, &b);
    assert!(result.abs() < 1e-6, "orthogonal vectors should return 0.0, got {result}");
}

#[test]
fn zero_vector_returns_zero() {
    let a = vec![0.0f32, 0.0, 0.0];
    let b = vec![1.0f32, 2.0, 3.0];
    assert_eq!(cosine_similarity(&a, &b), 0.0);
    assert_eq!(cosine_similarity(&b, &a), 0.0);
    assert_eq!(cosine_similarity(&a, &a), 0.0);
}

#[test]
fn empty_vectors_return_zero() {
    let empty: Vec<f32> = vec![];
    assert_eq!(cosine_similarity(&empty, &empty), 0.0);
}

#[test]
fn mismatched_length_returns_zero() {
    let a = vec![1.0f32, 2.0, 3.0];
    let b = vec![1.0f32, 2.0];
    assert_eq!(cosine_similarity(&a, &b), 0.0);
    assert_eq!(cosine_similarity(&b, &a), 0.0);
}

#[test]
fn known_non_trivial_pair() {
    // [1, 1] and [1, 0]: cos(45°) = 1/√2 ≈ 0.7071
    let a = vec![1.0f32, 1.0];
    let b = vec![1.0f32, 0.0];
    let expected = 1.0f32 / 2.0f32.sqrt();
    let result = cosine_similarity(&a, &b);
    assert!(
        (result - expected).abs() < 1e-6,
        "expected {expected}, got {result}"
    );
}

// ── embedding round-trip ──────────────────────────────────────────────────────

#[test]
fn embedding_round_trip_preserves_all_values() {
    let original = vec![0.0f32, 1.0, -1.0, f32::MAX, f32::MIN_POSITIVE, 3.14159];
    let bytes = embedding_to_bytes(&original);
    let recovered = bytes_to_embedding(&bytes);
    assert_eq!(original.len(), recovered.len());
    for (a, b) in original.iter().zip(recovered.iter()) {
        assert_eq!(a.to_bits(), b.to_bits(), "bit-exact round-trip failed: {a} != {b}");
    }
}

#[test]
fn embedding_round_trip_empty() {
    let original: Vec<f32> = vec![];
    let bytes = embedding_to_bytes(&original);
    let recovered = bytes_to_embedding(&bytes);
    assert!(recovered.is_empty());
}

#[test]
fn embedding_bytes_length_is_four_times_dims() {
    let v = vec![1.0f32, 2.0, 3.0];
    assert_eq!(embedding_to_bytes(&v).len(), 12);
}

#[test]
fn bytes_to_embedding_ignores_trailing_partial_chunk() {
    // 9 bytes = 2 complete f32s + 1 leftover byte (ignored by chunks_exact)
    let bytes = vec![0u8; 9];
    let v = bytes_to_embedding(&bytes);
    assert_eq!(v.len(), 2);
}

// ── hash_chunk ────────────────────────────────────────────────────────────────

#[test]
fn hash_chunk_is_deterministic() {
    let text = "hello world this is a deterministic hash test";
    assert_eq!(hash_chunk(text), hash_chunk(text));
}

#[test]
fn hash_chunk_different_inputs_differ() {
    let h1 = hash_chunk("alpha");
    let h2 = hash_chunk("beta");
    assert_ne!(h1, h2);
}

#[test]
fn hash_chunk_output_is_16_hex_chars() {
    let h = hash_chunk("some text");
    assert_eq!(h.len(), 16, "expected 16-char hex string, got {h:?}");
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "non-hex char in {h:?}");
}

#[test]
fn hash_chunk_empty_string_is_stable() {
    let h1 = hash_chunk("");
    let h2 = hash_chunk("");
    assert_eq!(h1, h2);
}
