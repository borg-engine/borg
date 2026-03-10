import { useEffect, useMemo, useRef } from "react";
import type { StreamEvent } from "@/lib/api";
import { parseStreamEvents } from "@/lib/stream-utils";
import { ActionActivity } from "./action-card";

interface LiveTerminalProps {
  events: StreamEvent[];
  streaming: boolean;
  title?: string;
  phase?: string;
}

export function LiveTerminal({ events, streaming, title, phase }: LiveTerminalProps) {
  const bottomRef = useRef<HTMLDivElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const autoScrollRef = useRef(true);

  const lines = useMemo(() => parseStreamEvents(events), [events]);

  useEffect(() => {
    if (autoScrollRef.current && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: "instant" });
    }
  }, []);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const onScroll = () => {
      const atBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
      autoScrollRef.current = atBottom;
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  return (
    <div className="flex flex-col h-full overflow-hidden">
      <div ref={containerRef} className="flex-1 overflow-y-auto overscroll-contain p-3">
        {lines.length === 0 && !streaming && (
          <div className="flex items-center justify-center py-8 text-[#6b6459] text-[13px]">
            No live stream available
          </div>
        )}
        {lines.length === 0 && streaming && (
          <div className="flex items-center justify-center py-8 text-[#6b6459] animate-pulse">
            Connecting to agent...
          </div>
        )}
        {lines.length > 0 && <ActionActivity lines={lines} streaming={streaming} title={title} phase={phase} />}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
