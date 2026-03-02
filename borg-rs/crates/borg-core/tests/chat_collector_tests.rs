use borg_core::chat::{ChatCollector, IncomingMessage};
use std::time::Duration;
use tokio::time::sleep;

fn msg(chat_key: &str, text: &str) -> IncomingMessage {
    IncomingMessage {
        chat_key: chat_key.to_string(),
        sender_name: "user".to_string(),
        text: text.to_string(),
        timestamp: 0,
        reply_to_message_id: None,
    }
}

/// Multiple expired collecting windows are all flushed in a single call.
#[tokio::test]
async fn test_flush_multiple_expired_windows() {
    let c = ChatCollector::new(1, 10, 0);

    assert!(c.process(msg("chat1", "hello")).await.is_none());
    assert!(c.process(msg("chat2", "world")).await.is_none());

    sleep(Duration::from_millis(20)).await;

    let batches = c.flush_expired().await;
    assert_eq!(batches.len(), 2);

    let keys: std::collections::HashSet<&str> =
        batches.iter().map(|b| b.chat_key.as_str()).collect();
    assert!(keys.contains("chat1"));
    assert!(keys.contains("chat2"));
    assert_eq!(c.active_count().await, 2);
}

/// A window that has not yet expired is left in Collecting and not returned.
#[tokio::test]
async fn test_non_expired_window_left_untouched() {
    let c = ChatCollector::new(60_000, 10, 0);

    assert!(c.process(msg("chat1", "hello")).await.is_none());

    let batches = c.flush_expired().await;
    assert!(batches.is_empty());
    assert_eq!(c.active_count().await, 0);

    // Confirm the chat is still Collecting by verifying a subsequent message
    // appends rather than re-opening a new window (no batch returned).
    assert!(c.process(msg("chat1", "still here")).await.is_none());
}

/// An expired Cooldown entry transitions to Idle, enabling future dispatches.
#[tokio::test]
async fn test_cooldown_expiry_transitions_to_idle() {
    // window_ms=0 for immediate dispatch, cooldown_ms=1
    let c = ChatCollector::new(0, 10, 1);

    let batch = c.process(msg("chat1", "hi")).await;
    assert!(batch.is_some());
    assert_eq!(c.active_count().await, 1);

    c.mark_done("chat1").await;
    assert_eq!(c.active_count().await, 0);

    sleep(Duration::from_millis(20)).await;

    // flush_expired transitions Cooldown → Idle; no batch emitted.
    let batches = c.flush_expired().await;
    assert!(batches.is_empty());

    // Chat is now Idle: a new message dispatches immediately.
    let batch2 = c.process(msg("chat1", "back again")).await;
    assert!(batch2.is_some());
    assert_eq!(batch2.unwrap().messages, vec!["back again"]);
}

/// The at-limit guard prevents dispatching beyond max_agents within one flush call,
/// and also prevents dispatch on subsequent calls while at the limit.
#[tokio::test]
async fn test_at_limit_guard_prevents_second_batch() {
    let c = ChatCollector::new(1, 1, 0);

    assert!(c.process(msg("chat_a", "first")).await.is_none());
    assert!(c.process(msg("chat_b", "second")).await.is_none());

    sleep(Duration::from_millis(20)).await;

    // Both windows are expired, but max_agents=1 so only one is dispatched.
    let batches = c.flush_expired().await;
    assert_eq!(batches.len(), 1);
    assert_eq!(c.active_count().await, 1);

    // The second chat's window is still expired but we are at the limit now.
    let batches2 = c.flush_expired().await;
    assert!(batches2.is_empty());
    assert_eq!(c.active_count().await, 1);
}

/// When running already equals max_agents before flush, no expired window is dispatched.
#[tokio::test]
async fn test_at_limit_from_start_blocks_all_flushes() {
    // Use a two-slot collector; fill both slots, add a third expired window.
    let c = ChatCollector::new(1, 2, 0);

    assert!(c.process(msg("chat_a", "a")).await.is_none());
    assert!(c.process(msg("chat_b", "b")).await.is_none());
    assert!(c.process(msg("chat_c", "c")).await.is_none());

    sleep(Duration::from_millis(20)).await;

    // First flush dispatches exactly max_agents=2.
    let batches = c.flush_expired().await;
    assert_eq!(batches.len(), 2);
    assert_eq!(c.active_count().await, 2);

    // Now at the limit; chat_c is still Collecting with expired deadline but blocked.
    let batches2 = c.flush_expired().await;
    assert!(batches2.is_empty());
    assert_eq!(c.active_count().await, 2);
}
