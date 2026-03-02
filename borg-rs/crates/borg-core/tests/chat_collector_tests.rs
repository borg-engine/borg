use std::time::Duration;
use borg_core::chat::{ChatCollector, IncomingMessage};

fn msg(chat_key: &str, text: &str) -> IncomingMessage {
    IncomingMessage {
        chat_key: chat_key.to_string(),
        sender_name: "alice".to_string(),
        text: text.to_string(),
        timestamp: 0,
        reply_to_message_id: None,
    }
}

#[tokio::test]
async fn test_window_zero_immediate_dispatch() {
    let c = ChatCollector::new(0, 0, 0);
    let batch = c.process(msg("chat1", "hello")).await;
    assert!(batch.is_some());
    let b = batch.unwrap();
    assert_eq!(b.chat_key, "chat1");
    assert_eq!(b.sender_name, "alice");
    assert_eq!(b.messages, vec!["hello"]);
}

#[tokio::test]
async fn test_window_nonzero_defers_dispatch() {
    let c = ChatCollector::new(5000, 0, 0);
    let result = c.process(msg("chat1", "hi")).await;
    assert!(result.is_none(), "should not dispatch while window is open");
    assert_eq!(c.active_count().await, 0);
}

#[tokio::test]
async fn test_collecting_batches_multiple_messages() {
    let c = ChatCollector::new(5000, 0, 0);
    // All three messages arrive before the 5s window closes
    assert!(c.process(msg("chat1", "msg1")).await.is_none());
    assert!(c.process(msg("chat1", "msg2")).await.is_none());
    assert!(c.process(msg("chat1", "msg3")).await.is_none());
    // Nothing dispatched yet
    assert_eq!(c.active_count().await, 0);
    let ready = c.flush_expired().await;
    assert!(ready.is_empty(), "window not expired, nothing should flush");
}

#[tokio::test]
async fn test_flush_expired_dispatches_after_window() {
    let c = ChatCollector::new(1, 0, 0);
    assert!(c.process(msg("chat1", "first")).await.is_none());
    assert!(c.process(msg("chat1", "second")).await.is_none());
    // Wait for window to expire
    tokio::time::sleep(Duration::from_millis(10)).await;
    let ready = c.flush_expired().await;
    assert_eq!(ready.len(), 1);
    let b = &ready[0];
    assert_eq!(b.chat_key, "chat1");
    assert_eq!(b.messages, vec!["first", "second"]);
    assert_eq!(c.active_count().await, 1);
}

#[tokio::test]
async fn test_flush_no_dispatch_before_window_expires() {
    let c = ChatCollector::new(5000, 0, 0);
    assert!(c.process(msg("chat1", "hello")).await.is_none());
    let ready = c.flush_expired().await;
    assert!(ready.is_empty());
}

#[tokio::test]
async fn test_running_state_drops_messages() {
    let c = ChatCollector::new(0, 0, 0);
    // First message dispatches and puts chat into Running
    let batch = c.process(msg("chat1", "msg1")).await;
    assert!(batch.is_some());
    // Subsequent messages on same chat are dropped
    assert!(c.process(msg("chat1", "msg2")).await.is_none());
    assert!(c.process(msg("chat1", "msg3")).await.is_none());
    assert_eq!(c.active_count().await, 1);
}

#[tokio::test]
async fn test_can_dispatch_gated_by_max_agents() {
    let c = ChatCollector::new(0, 1, 0); // max 1 agent
    assert!(c.can_dispatch().await);
    // Dispatch to chat1
    assert!(c.process(msg("chat1", "go")).await.is_some());
    // Now at limit
    assert!(!c.can_dispatch().await);
    // chat2 cannot be dispatched
    assert!(c.process(msg("chat2", "go")).await.is_none());
    // After chat1 finishes, capacity opens
    c.mark_done("chat1").await;
    assert!(c.can_dispatch().await);
    assert_eq!(c.active_count().await, 0);
    // chat2 can now dispatch
    assert!(c.process(msg("chat2", "go")).await.is_some());
}

#[tokio::test]
async fn test_max_agents_zero_means_unlimited() {
    let c = ChatCollector::new(0, 0, 0); // max_agents=0 → unlimited
    for i in 0..10 {
        let key = format!("chat{i}");
        assert!(c.process(msg(&key, "go")).await.is_some());
    }
    assert_eq!(c.active_count().await, 10);
}

#[tokio::test]
async fn test_mark_done_with_cooldown_blocks_messages() {
    let c = ChatCollector::new(0, 0, 5000); // 5s cooldown
    let batch = c.process(msg("chat1", "msg1")).await;
    assert!(batch.is_some());
    c.mark_done("chat1").await;
    assert_eq!(c.active_count().await, 0);
    // Chat is now in Cooldown — messages are dropped
    assert!(c.process(msg("chat1", "msg2")).await.is_none());
}

#[tokio::test]
async fn test_mark_done_no_cooldown_returns_to_idle() {
    let c = ChatCollector::new(0, 0, 0); // no cooldown
    assert!(c.process(msg("chat1", "msg1")).await.is_some());
    c.mark_done("chat1").await;
    assert_eq!(c.active_count().await, 0);
    // Should accept new messages immediately (back to Idle)
    let batch = c.process(msg("chat1", "msg2")).await;
    assert!(batch.is_some());
    assert_eq!(batch.unwrap().messages, vec!["msg2"]);
}

#[tokio::test]
async fn test_cooldown_expires_via_flush_restores_idle() {
    let c = ChatCollector::new(0, 0, 1); // 1ms cooldown
    assert!(c.process(msg("chat1", "msg1")).await.is_some());
    c.mark_done("chat1").await;
    // In cooldown — message dropped
    assert!(c.process(msg("chat1", "early")).await.is_none());
    // Wait for cooldown to expire
    tokio::time::sleep(Duration::from_millis(10)).await;
    c.flush_expired().await; // transitions Cooldown → Idle
    // Now accepts messages
    let batch = c.process(msg("chat1", "after")).await;
    assert!(batch.is_some());
    assert_eq!(batch.unwrap().messages, vec!["after"]);
}

#[tokio::test]
async fn test_full_state_cycle() {
    // Idle → Collecting → Running → Cooldown → Idle
    let c = ChatCollector::new(1, 0, 1);
    // Idle → Collecting
    assert!(c.process(msg("chat1", "a")).await.is_none());
    assert!(c.process(msg("chat1", "b")).await.is_none());
    // Wait for collection window
    tokio::time::sleep(Duration::from_millis(10)).await;
    // Collecting → Running via flush
    let ready = c.flush_expired().await;
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].messages, vec!["a", "b"]);
    assert_eq!(c.active_count().await, 1);
    // Running → Cooldown via mark_done
    c.mark_done("chat1").await;
    assert_eq!(c.active_count().await, 0);
    // In cooldown, messages dropped
    assert!(c.process(msg("chat1", "c")).await.is_none());
    // Wait for cooldown
    tokio::time::sleep(Duration::from_millis(10)).await;
    c.flush_expired().await;
    // Back to Idle — new message starts another collection window
    assert!(c.process(msg("chat1", "d")).await.is_none());
    tokio::time::sleep(Duration::from_millis(10)).await;
    let ready2 = c.flush_expired().await;
    assert_eq!(ready2.len(), 1);
    assert_eq!(ready2[0].messages, vec!["d"]);
}

#[tokio::test]
async fn test_multiple_chats_independent() {
    let c = ChatCollector::new(5000, 0, 0);
    // Two chats collect independently
    assert!(c.process(msg("chat1", "hello")).await.is_none());
    assert!(c.process(msg("chat2", "world")).await.is_none());
    assert!(c.process(msg("chat1", "there")).await.is_none());
    // Neither dispatched
    assert_eq!(c.active_count().await, 0);
    let ready = c.flush_expired().await;
    assert!(ready.is_empty());
}

#[tokio::test]
async fn test_flush_expired_respects_max_agents() {
    let c = ChatCollector::new(1, 1, 0); // max 1 agent, 1ms window
    assert!(c.process(msg("chat1", "a")).await.is_none());
    assert!(c.process(msg("chat2", "b")).await.is_none());
    tokio::time::sleep(Duration::from_millis(10)).await;
    let ready = c.flush_expired().await;
    // Only one batch dispatched (max_agents=1)
    assert_eq!(ready.len(), 1);
    assert_eq!(c.active_count().await, 1);
}
