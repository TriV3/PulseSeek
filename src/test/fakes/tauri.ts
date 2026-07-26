import type { Event as TauriEvent, UnlistenFn } from "@tauri-apps/api/event";

type CommandRequest<
  Commands,
  Command extends keyof Commands,
> = Commands[Command] extends { request: infer Request } ? Request : never;

type CommandResponse<
  Commands,
  Command extends keyof Commands,
> = Commands[Command] extends { response: infer Response } ? Response : never;

export interface TauriCommandPort<Commands> {
  invoke<Command extends keyof Commands>(
    command: Command,
    request: CommandRequest<Commands, Command>,
  ): Promise<CommandResponse<Commands, Command>>;
}

export interface TauriEventPort<Events> {
  listen<Event extends keyof Events>(
    event: Event,
    handler: (event: TauriEvent<Events[Event]>) => void,
  ): Promise<UnlistenFn>;
}

export class FakeTauriCommands<Commands> implements TauriCommandPort<Commands> {
  readonly calls: Array<{
    command: keyof Commands;
    request: unknown;
  }> = [];

  private readonly responses = new Map<keyof Commands, unknown>();

  respond<Command extends keyof Commands>(
    command: Command,
    response: CommandResponse<Commands, Command>,
  ): void {
    this.responses.set(command, response);
  }

  async invoke<Command extends keyof Commands>(
    command: Command,
    request: CommandRequest<Commands, Command>,
  ): Promise<CommandResponse<Commands, Command>> {
    this.calls.push({ command, request });

    if (!this.responses.has(command)) {
      throw new Error(`No fake response configured for ${String(command)}`);
    }

    return this.responses.get(command) as CommandResponse<Commands, Command>;
  }
}

export class FakeTauriEvents<Events> implements TauriEventPort<Events> {
  private nextEventId = 1;
  private readonly handlers = new Map<
    keyof Events,
    Set<(event: TauriEvent<Events[keyof Events]>) => void>
  >();

  async listen<Event extends keyof Events>(
    event: Event,
    handler: (event: TauriEvent<Events[Event]>) => void,
  ): Promise<UnlistenFn> {
    const handlers = this.handlers.get(event) ?? new Set();
    handlers.add(handler as (event: TauriEvent<Events[keyof Events]>) => void);
    this.handlers.set(event, handlers);

    return () => {
      handlers.delete(
        handler as (event: TauriEvent<Events[keyof Events]>) => void,
      );
    };
  }

  emit<Event extends keyof Events>(event: Event, payload: Events[Event]): void {
    const emittedEvent: TauriEvent<Events[Event]> = {
      event: String(event),
      id: this.nextEventId,
      payload,
    };
    this.nextEventId += 1;

    for (const handler of this.handlers.get(event) ?? []) {
      handler(emittedEvent);
    }
  }
}
