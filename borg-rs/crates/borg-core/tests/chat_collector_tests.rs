use borg_core::chat::{ChatCollector, IncomingMessage};

fn make_msg(chat_key: &str, text: &str) -> IncomingMessage {
    IncomingMessage {
        chat_key: chat_key.to_string(),
        sender_name: "alice".to_string(),
        text: text.to_string(),
        timestamp: 0,
        reply_to_message_id: None,
    }
}

// Idle + window_ms=0: dispatches immediately
#[tokio::test]
async fn idle_zero_window_dispatches_immediately() {
    let collector = ChatCollector::new(0, 1, 0);
    let batch = collector.process(make_msg("chat:1", "hello")).await;
    assert!(batch.is_some());
    let batch = batch.unwrap();
    assert_eq!(batch.chat_key, "chat:1");
    assert_eq!(batch.messages, vec!["hello"]);
    assert_eq!(batch.sender_name, "alice");
}

// Idle + window_ms>0: enters Collecting, no dispatch
#[tokio::test]
async fn idle_nonzero_window_enters_collecting() {
    let collector = ChatCollector::new(60_000, 1, 0);
    let result = collector.process(make_msg("chat:1", "hello")).await;
    assert!(result.is_none());
    // Still no agents running
    assert_eq!(collector.active_count().await, 0);
}

// Collecting before deadline: accumulates messages, no dispatch
#[tokio::test]
async fn collecting_before_deadline_accumulates() {
    let collector = ChatCollector::new(60_000, 1, 0);
    // First message enters Collecting
    let r1 = collector.process(make_msg("chat:1", "first")).await;
    assert!(r1.is_none());
    // Second message is accumulated
    let r2 = collector.process(make_msg("chat:1", "second")).await;
    assert!(r2.is_none());
    assert_eq!(collector.active_count().await, 0);
}

// Collecting after deadline: dispatches the batch with all messages
#[tokio::test]
async fn collecting_after_deadline_dispatches_batch() {
    let collector = ChatCollector::new(1, 1, 0); // 1ms window
    // First message enters Collecting
    let r1 = collector.process(make_msg("chat:1", "first")).await;
    assert!(r1.is_none());

    // Wait for the window to expire
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;

    // Second message arrives after deadline and triggers dispatch
    let batch = collector.process(make_msg("chat:1", "second")).await;
    assert!(batch.is_some());
    let batch = batch.unwrap();
    assert_eq!(batch.chat_key, "chat:1");
    // Both messages are in the batch (first was stored, second was pushed before dispatch)
    assert_eq!(batch.messages, vec!["first", "second"]);
}

// Running: drops the message
#[tokio::test]
async fn running_drops_message() {
    let collector = ChatCollector::new(0, 1, 0);
    // Dispatch puts chat into Running
    let batch = collector.process(make_msg("chat:1", "go")).await;
    assert!(batch.is_some());
    assert_eq!(collector.active_count().await, 1);

    // Subsequent message is dropped
    let dropped = collector.process(make_msg("chat:1", "another")).await;
    assert!(dropped.is_none());
    assert_eq!(collector.active_count().await, 1);
}

// Cooldown: drops the message
#[tokio::test]
async fn cooldown_drops_message() {
    let collector = ChatCollector::new(0, 1, 60_000); // 60s cooldown
    // Dispatch
    let batch = collector.process(make_msg("chat:1", "go")).await;
    assert!(batch.is_some());

    // Mark done → enters Cooldown
    collector.mark_done("chat:1").await;
    assert_eq!(collector.active_count().await, 0);

    // Message during cooldown is dropped
    let dropped = collector.process(make_msg("chat:1", "during cooldown")).await;
    assert!(dropped.is_none());
}
