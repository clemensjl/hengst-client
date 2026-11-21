import { Channel, invoke } from "@tauri-apps/api/core";
import { guard } from "./client";
import type { AppEvent } from "./bindings/AppEvent";

/** Replaces any earlier subscription: the backend keeps one subscriber. */
export function subscribeEvents(onEvent: (e: AppEvent) => void): Promise<void> {
  const channel = new Channel<AppEvent>();
  channel.onmessage = onEvent;
  return guard(invoke<void>("subscribe_events", { channel }));
}
