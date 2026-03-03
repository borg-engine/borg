use std::time::Duration;

use borg_core::chat::{ChatCollector, IncomingMessage};

fn make_msg(chat_key: &str, text: &str) -> IncomingMessage {
    IncomingMessage {
        chat_key: chat_key.to_string(),
        sender_name: "tester".to_string(),
        text: text.to_string(),
        timestamp: 0,
        reply_to_message_id: None,
    }
}

// Messages arriving within the collection window are batched together.
#[tokio::test]
async fn messages_batched_within_window() {
    let collector = ChatCollector::new(50, 0, 0);

    let r1 = collector.process(make_msg("chat:1", "hello")).await;
    assert!(r1.is_none(), "first message should open window, not dispatch");

    let r2 = collector.process(make_msg("chat:1", "world")).await;
    assert!(r2.is_none(), "second message within window should not dispatch");

    tokio::time::sleep(Duration::from_millis(60)).await;
    let batches = collector.flush_expired().await;

    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].chat_key, "chat:1");
    assert_eq!(batches[0].messages, vec!["hello", "world"]);
}

// Expired collection window triggers dispatch via flush_expired().
#[tokio::test]
async fn window_expiry_triggers_dispatch() {
    let collector = ChatCollector::new(1, 0, 0);

    let r = collector.process(make_msg("chat:2", "ping")).await;
    assert!(r.is_none(), "message should open collection window");

    tokio::time::sleep(Duration::from_millis(10)).await;
    let batches = collector.flush_expired().await;

    assert_eq!(batches.len(), 1, "expired window should yield one batch");
    assert_eq!(batches[0].chat_key, "chat:2");
    assert_eq!(batches[0].messages, vec!["ping"]);
}

// Cooldown prevents dispatch of new messages until flush_expired clears it.
#[tokio::test]
async fn cooldown_prevents_dispatch() {
    let collector = ChatCollector::new(0, 0, 100);

    // window_ms=0: first message dispatches immediately
    let r = collector.process(make_msg("chat:3", "first")).await;
    assert!(r.is_some(), "immediate dispatch expected with window_ms=0");

    // Agent finishes → cooldown starts
    collector.mark_done("chat:3").await;

    // Message during cooldown is dropped
    let r2 = collector.process(make_msg("chat:3", "during cooldown")).await;
    assert!(r2.is_none(), "message during cooldown should be dropped");

    // Cooldown expires
    tokio::time::sleep(Duration::from_millis(110)).await;
    collector.flush_expired().await; // transitions chat:3 to Idle

    // Message after cooldown dispatches immediately (window_ms=0)
    let r3 = collector.process(make_msg("chat:3", "after cooldown")).await;
    assert!(r3.is_some(), "message after cooldown should dispatch");
}

// Concurrent-agent limit holds a message in the Collecting queue until a slot frees.
#[tokio::test]
async fn concurrent_agent_limit_holds_message_in_queue() {
    let collector = ChatCollector::new(1, 1, 0); // window=1ms, max_agents=1

    // Open collection windows for two chats simultaneously
    assert!(collector.process(make_msg("chat:A", "msg A")).await.is_none());
    assert!(collector.process(make_msg("chat:B", "msg B")).await.is_none());

    // Both windows expire
    tokio::time::sleep(Duration::from_millis(10)).await;

    // First flush: only one chat can dispatch (max_agents=1)
    let batches1 = collector.flush_expired().await;
    assert_eq!(batches1.len(), 1, "only one dispatch allowed at max_agents=1");
    assert_eq!(collector.active_count().await, 1);

    // Second flush while at limit: the other chat stays queued
    let batches2 = collector.flush_expired().await;
    assert_eq!(batches2.len(), 0, "no dispatch while at concurrency limit");

    // Release the running agent
    collector.mark_done(&batches1[0].chat_key).await;
    assert_eq!(collector.active_count().await, 0);

    // Now the held chat dispatches
    let batches3 = collector.flush_expired().await;
    assert_eq!(batches3.len(), 1, "held chat should dispatch after slot freed");
}
