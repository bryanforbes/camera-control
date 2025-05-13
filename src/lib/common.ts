import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { listen as tauriListen } from '@tauri-apps/api/event';

export interface UIState {
  port: string | null;
  ports: readonly string[] | null;
  status: string;
}

interface InvokeSignature<A = never, R = void> {
  args: [A] extends [never] ? [] : [A];
  return: R;
}

interface InvokeMap {
  open_settings: InvokeSignature;
  stop_move: InvokeSignature;
  stop_zoom: InvokeSignature;
  set_port: InvokeSignature<{ portName: string | null }>;
  camera_power: InvokeSignature<{ power: boolean }>;
  autofocus: InvokeSignature<{ autofocus: boolean }>;
  go_to_preset: InvokeSignature<{ preset: number; name: string }>;
  set_preset: InvokeSignature<{ preset: number; name: string }>;
  move_camera: InvokeSignature<{ direction: string }>;
  zoom: InvokeSignature<{ direction: string }>;
  get_state: InvokeSignature<never, UIState>;
}

export const invoke: <K extends keyof InvokeMap>(
  cmd: K,
  ...args: InvokeMap[K]['args']
) => Promise<InvokeMap[K]['return']> = tauriInvoke;

interface ListenMap {
  'ui-state': UIState;
  status: string;
}

export const listen: <K extends keyof ListenMap>(
  event: K,
  handler: EventCallback<ListenMap[K]>,
) => Promise<UnlistenFn> = tauriListen;
