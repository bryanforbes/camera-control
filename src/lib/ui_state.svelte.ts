import { commands, events, type UIStateEvent } from './bindings';

const _ui_state: UIStateEvent = $state({
  port: null,
  ports: null,
  status: 'Disconnected',
});

void commands.getState().then(({ port, ports, status }) => {
  _ui_state.port = port;
  _ui_state.ports = ports;
  _ui_state.status = status;
});

void events.uiStateEvent.listen(({ payload: { port, ports, status } }) => {
  _ui_state.port = port;
  _ui_state.ports = ports;
  _ui_state.status = status;
});

export const ui_state: Readonly<UIStateEvent> = _ui_state;
