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
