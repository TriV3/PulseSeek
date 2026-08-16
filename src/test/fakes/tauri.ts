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

type EventHandler<Events> = (event: TauriEvent<Events[keyof Events]>) => void;

type EventRegistration<Events> = {
  id: number;
  handler: EventHandler<Events>;
};

export class FakeTauriEvents<Events> implements TauriEventPort<Events> {
  private nextListenerId = 1;
  private readonly registrations = new Map<
    keyof Events,
    Array<EventRegistration<Events>>
  >();

  async listen<Event extends keyof Events>(
    event: Event,
    handler: (event: TauriEvent<Events[Event]>) => void,
  ): Promise<UnlistenFn> {
    const registrations = this.registrations.get(event) ?? [];
    const registration = {
      id: this.nextListenerId,
      handler: handler as EventHandler<Events>,
    };
    this.nextListenerId += 1;
    registrations.push(registration);
    this.registrations.set(event, registrations);

    return () => {
      const index = registrations.indexOf(registration);
      if (index >= 0) {
        registrations.splice(index, 1);
      }
    };
  }

  emit<Event extends keyof Events>(event: Event, payload: Events[Event]): void {
    for (const registration of this.registrations.get(event) ?? []) {
      registration.handler({
        event: String(event),
        id: registration.id,
        payload,
      });
    }
  }
}
