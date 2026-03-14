import { useCallback, useEffect, useRef, useState } from "react";
import type { StreamEvent } from "./api";
import { AuthEventSource, tokenReady } from "./api";

/**
 * Per-thread chat stream hook. Connects to /api/chat/threads/:thread/stream
 * and maintains streamEvents state with full history replay on connect/reconnect.
 *
 * On reconnect, streamEvents is replaced wholesale with the full history —
 * no gaps, no merge logic, no missing action cards.
 */
export function useChatStream(thread: string | null) {
  const [streamEvents, setStreamEvents] = useState<StreamEvent[]>([]);
  const [isStreaming, setIsStreaming] = useState(false);
  const esRef = useRef<AuthEventSource | null>(null);
  const retriesRef = useRef(0);
  const retryTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastEventTimeRef = useRef(0);
  // Track whether we've received any history/live events in the current connection
  const receivedEventsRef = useRef(false);

  const reset = useCallback(() => {
    setStreamEvents([]);
    setIsStreaming(false);
    receivedEventsRef.current = false;
  }, []);

  useEffect(() => {
    if (!thread) {
      reset();
      return;
    }

    let cancelled = false;
    // Accumulated events during a single connection — replaced on reconnect
    let buffer: StreamEvent[] = [];

    function connect() {
      if (cancelled) return;
      if (esRef.current) esRef.current.close();

      // Reset buffer on each connect — full replay means we start fresh
      buffer = [];
      receivedEventsRef.current = false;

      tokenReady.then(() => {
        if (cancelled) return;
        const encodedThread = encodeURIComponent(thread!);
        const es = new AuthEventSource(
          `/api/chat/threads/${encodedThread}/stream`,
        );
        esRef.current = es;

        es.onopen = () => {
          retriesRef.current = 0;
        };

        es.onmessage = (e) => {
          try {
            const wrapper = JSON.parse(e.data);
            if (wrapper.type !== "chat_stream" || !wrapper.data) return;

            const parsed: StreamEvent =
              typeof wrapper.data === "string"
                ? JSON.parse(wrapper.data)
                : wrapper.data;

            if (!parsed.type) return;

            // stream_end sentinel — mark streaming as done
            if (parsed.type === "stream_end") {
              setIsStreaming(false);
              return;
            }

            receivedEventsRef.current = true;
            setIsStreaming(true);
            lastEventTimeRef.current = Date.now();

            buffer.push(parsed);
            // Update state with a fresh copy of buffer
            setStreamEvents([...buffer]);
          } catch {
            // ignore malformed events
          }
        };

        es.onerror = () => {
          es.close();
          esRef.current = null;
          if (!cancelled && retriesRef.current < 20) {
            const delay = Math.min(1000 * 2 ** retriesRef.current, 30_000);
            retriesRef.current++;
            retryTimerRef.current = setTimeout(connect, delay);
          }
        };
      });
    }

    connect();

    return () => {
      cancelled = true;
      esRef.current?.close();
      esRef.current = null;
      if (retryTimerRef.current) clearTimeout(retryTimerRef.current);
    };
  }, [thread, reset]);

  return { streamEvents, isStreaming, lastEventTimeRef, reset };
}
