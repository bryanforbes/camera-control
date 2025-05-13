import { invoke, listen, type UIState } from './common';

const _ui_state: UIState = $state({
  port: null,
  ports: null,
  status: 'Disconnected',
});

void invoke('get_state').then(({ port, ports, status }) => {
  _ui_state.port = port;
  _ui_state.ports = ports;
  _ui_state.status = status;
});

void listen('ui-state', ({ payload: { port, ports, status } }) => {
  _ui_state.port = port;
  _ui_state.ports = ports;
  _ui_state.status = status;
});

export const ui_state: Readonly<UIState> = _ui_state;
