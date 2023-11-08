import type { EventName, UnlistenFn } from '@tauri-apps/api/event';
import { listen as tauriListen } from '@tauri-apps/api/event';
import { invoke as tauriInvoke, type InvokeArgs } from '@tauri-apps/api/tauri';

export interface PortState {
  port: string | null;
}

export async function invoke<T>(
  cmd: string,
  args?: InvokeArgs,
  updateStatus?: () => Promise<void> | void,
): Promise<T> {
  const result = await tauriInvoke<T>(cmd, args);

  if (updateStatus) {
    await updateStatus();
  }

  return result;
}

export async function listen<T>(
  event: EventName,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  return tauriListen<T>(event, ({ payload }) => handler(payload));
}

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
