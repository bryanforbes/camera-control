import type { EventCallback, UnlistenFn } from '@tauri-apps/api/event';
import { listen as tauriListen } from '@tauri-apps/api/event';
import { invoke as tauriInvoke } from '@tauri-apps/api/tauri';

export interface PortState {
  port: string | null;
}

interface InvokeSignature<A = never, R = void> {
  args: [A] extends [never] ? [] : [A];
  return: R;
}

interface InvokeMap {
  open_settings: InvokeSignature;
  ready: InvokeSignature;
  stop_move: InvokeSignature;
  stop_zoom: InvokeSignature;
  set_port: InvokeSignature<{ portName: string | null }>;
  camera_power: InvokeSignature<{ power: boolean }>;
  autofocus: InvokeSignature<{ autofocus: boolean }>;
  go_to_preset: InvokeSignature<{ preset: number }>;
  set_preset: InvokeSignature<{ preset: number }>;
  move_camera: InvokeSignature<{ direction: string }>;
  zoom: InvokeSignature<{ direction: string }>;
  get_ports: InvokeSignature<never, string[]>;
}

export const invoke: <K extends keyof InvokeMap>(
  cmd: K,
  ...args: InvokeMap[K]['args']
) => Promise<InvokeMap[K]['return']> = tauriInvoke;

interface ListenMap {
  'port-state': PortState;
  status: string;
}

export const listen: <K extends keyof ListenMap>(
  event: K,
  handler: EventCallback<ListenMap[K]>,
) => Promise<UnlistenFn> = tauriListen;

export function toggleControls(parentSelector: string, enabled: boolean): void {
  const parent = document.querySelector<HTMLElement>(parentSelector);

  if (parent) {
    parent.inert = !enabled;
  }
}

type AsyncEventListener<T, E> = (this: T, ev: E) => Promise<unknown>;

export interface AddAsyncEventListener {
  <K extends keyof WindowEventMap>(
    target: Window | null | undefined,
    type: K,
    listener: AsyncEventListener<Window, WindowEventMap[K]>,
    options?: AddEventListenerOptions,
  ): void;
  <K extends keyof DocumentEventMap>(
    target: Document | null | undefined,
    type: K,
    listener: AsyncEventListener<Document, DocumentEventMap[K]>,
    options?: AddEventListenerOptions,
  ): void;
  <T extends HTMLElement, K extends keyof HTMLElementEventMap>(
    target: T | null | undefined,
    type: K,
    listener: AsyncEventListener<T, HTMLElementEventMap[K]>,
    options?: AddEventListenerOptions,
  ): void;
}

export function createAddAsyncEventListener(
  errorHandler: (error: unknown) => unknown,
): AddAsyncEventListener {
  return function addAsyncEventListener(
    target: EventTarget | null | undefined,
    type: string,
    listener: (this: EventTarget, ev: Event) => Promise<unknown>,
    options?: AddEventListenerOptions,
  ) {
    target?.addEventListener(
      type,
      function (this: EventTarget, event) {
        listener.call(this, event).catch(errorHandler);
      },
      options,
    );
  };
}
