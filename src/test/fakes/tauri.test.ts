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

  it("keeps a stable listener id across event emissions", async () => {
    const events = new FakeTauriEvents<Events>();
    const listener = vi.fn();
    const unsubscribe = await events.listen("status", listener);

    events.emit("status", { state: "ready" });
    events.emit("status", { state: "stopped" });
    unsubscribe();
    events.emit("status", { state: "ready" });

    expect(listener).toHaveBeenCalledTimes(2);
    expect(listener).toHaveBeenNthCalledWith(1, {
      event: "status",
      id: 1,
      payload: { state: "ready" },
    });
    expect(listener).toHaveBeenNthCalledWith(2, {
      event: "status",
      id: 1,
      payload: { state: "stopped" },
    });
  });

  it("unsubscribes duplicate callback registrations independently", async () => {
    const events = new FakeTauriEvents<Events>();
    const listener = vi.fn();
    const unsubscribeFirst = await events.listen("status", listener);
    await events.listen("status", listener);

    unsubscribeFirst();
    events.emit("status", { state: "ready" });

    expect(listener).toHaveBeenCalledOnce();
    expect(listener).toHaveBeenCalledWith({
      event: "status",
      id: 2,
      payload: { state: "ready" },
    });
  });
});
