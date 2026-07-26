import { describe, expect, it, vi } from "vitest";
import { FakeTauriCommands, FakeTauriEvents } from "./tauri";

type Commands = {
  health: {
    request: { version: number };
    response: { ready: boolean };
  };
};

type Events = {
  status: { state: "ready" | "stopped" };
};

describe("typed Tauri fakes", () => {
  it("records command requests and returns configured responses", async () => {
    const commands = new FakeTauriCommands<Commands>();
    commands.respond("health", { ready: true });

    await expect(commands.invoke("health", { version: 1 })).resolves.toEqual({
      ready: true,
    });
    expect(commands.calls).toEqual([
      { command: "health", request: { version: 1 } },
    ]);
  });

  it("emits events until the listener unsubscribes", async () => {
    const events = new FakeTauriEvents<Events>();
    const listener = vi.fn();
    const unsubscribe = await events.listen("status", listener);

    events.emit("status", { state: "ready" });
    unsubscribe();
    events.emit("status", { state: "stopped" });

    expect(listener).toHaveBeenCalledOnce();
    expect(listener).toHaveBeenCalledWith({
      event: "status",
      id: 1,
      payload: { state: "ready" },
    });
  });
});
