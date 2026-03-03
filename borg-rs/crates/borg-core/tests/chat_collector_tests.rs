use std::time::Duration;

use borg_core::chat::{ChatCollector, IncomingMessage};

fn make_msg(chat_key: &str) -> IncomingMessage {
    IncomingMessage {
        chat_key: chat_key.to_string(),
        sender_name: "Alice".to_string(),
        text: "hello".to_string(),
        timestamp: 0,
        reply_to_message_id: None,
    }
}

#[tokio::test]
async fn flush_dispatches_expired_batch() {
    let collector = ChatCollector::new(1, 10, 0);
    let result = collector.process(make_msg("chat1")).await;
    assert!(result.is_none(), "first process should start collecting");

    tokio::time::sleep(Duration::from_millis(5)).await;

    let batches = collector.flush_expired().await;
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].chat_key, "chat1");
    assert_eq!(collector.active_count().await, 1);
}

#[tokio::test]
async fn flush_skips_unexpired_batch() {
    let collector = ChatCollector::new(60_000, 10, 0);
    let result = collector.process(make_msg("chat1")).await;
    assert!(result.is_none());

    let batches = collector.flush_expired().await;
    assert!(batches.is_empty());
    assert_eq!(collector.active_count().await, 0);
}

#[tokio::test]
async fn flush_respects_max_agents_leaves_second_pending() {
    let collector = ChatCollector::new(1, 1, 0);
    assert!(collector.process(make_msg("chat1")).await.is_none());
    assert!(collector.process(make_msg("chat2")).await.is_none());

    tokio::time::sleep(Duration::from_millis(5)).await;

    let first_batches = collector.flush_expired().await;
    assert_eq!(
        first_batches.len(),
        1,
        "only one batch should be dispatched at max_agents=1"
    );
    assert_eq!(collector.active_count().await, 1);

    // Mark the first agent done; the second chat should now be dispatchable.
    collector.mark_done(&first_batches[0].chat_key).await;
    let second_batches = collector.flush_expired().await;
    assert_eq!(
        second_batches.len(),
        1,
        "second chat should be dispatched after first completes"
    );
}

#[tokio::test]
async fn mark_done_with_cooldown_enters_cooldown() {
    let collector = ChatCollector::new(0, 10, 10_000);
    let result = collector.process(make_msg("chat1")).await;
    assert!(result.is_some(), "window=0 should dispatch immediately");
    assert_eq!(collector.active_count().await, 1);

    collector.mark_done("chat1").await;
    assert_eq!(collector.active_count().await, 0);

    // During cooldown, new messages must be dropped.
    let result = collector.process(make_msg("chat1")).await;
    assert!(result.is_none(), "message during cooldown should be dropped");
}

#[tokio::test]
async fn mark_done_without_cooldown_returns_idle() {
    let collector = ChatCollector::new(0, 10, 0);
    let result = collector.process(make_msg("chat1")).await;
    assert!(result.is_some());
    assert_eq!(collector.active_count().await, 1);

    collector.mark_done("chat1").await;
    assert_eq!(collector.active_count().await, 0);

    // Idle chat should dispatch immediately (window=0).
    let result = collector.process(make_msg("chat1")).await;
    assert!(
        result.is_some(),
        "after mark_done with no cooldown, chat should be Idle"
    );
}

#[tokio::test]
async fn mark_done_saturating_sub_no_underflow() {
    let collector = ChatCollector::new(0, 10, 0);
    assert_eq!(collector.active_count().await, 0);

    // Mark done without any running agent — must not underflow.
    collector.mark_done("ghost_chat").await;
    assert_eq!(collector.active_count().await, 0);
}
