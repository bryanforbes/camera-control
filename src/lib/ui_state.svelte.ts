import { commands, events, type UIStateEvent } from './bindings';

export interface ReadonlyUIStateEvent extends Readonly<Omit<UIStateEvent, 'ports'>> {
  readonly ports: readonly string[] | null;
}

const state: UIStateEvent = $state({
  port: null,
  ports: null,
  status: 'Disconnected',
});

function set({ port, ports, status }: UIStateEvent) {
  state.port = port;
  state.ports = ports;
  state.status = status;
}

void commands.getState().then(set);
void events.uiStateEvent.listen((event) => set(event.payload));

export const ui_state: ReadonlyUIStateEvent = state;
