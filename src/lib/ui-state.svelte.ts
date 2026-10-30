import { commands, events, type UIStateEvent_Serialize } from './bindings';

export interface ReadonlyUIStateEvent extends Readonly<Omit<UIStateEvent_Serialize, 'ports'>> {
  readonly ports: readonly string[] | null;
}

const state: UIStateEvent_Serialize = $state({
  port: null,
  ports: null,
  status: 'Disconnected',
});

function set({ port, ports, status }: UIStateEvent_Serialize) {
  state.port = port;
  state.ports = ports;
  state.status = status;
}

void commands.getState().then(set);
void events.uiStateEvent.listen((event) => set(event.payload));

export const uiState: ReadonlyUIStateEvent = state;
