import { message } from '@tauri-apps/api/dialog';
import { InvokeArgs, invoke as tauriInvoke } from '@tauri-apps/api/tauri';

export interface CameraState {
  power: boolean;
  autofocus: boolean;
  status: string;
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

export async function displayError(e: unknown): Promise<void> {
  await message(`${e}`, {
    title: 'Error',
    type: 'error',
  });
}

export function toggleControls(parentSelector: string, enabled: boolean): void {
  const parent = document.querySelector<HTMLElement>(parentSelector);

  if (parent) {
    parent.inert = !enabled;
  }
}

export function asyncListener<Event, Error>(
  listener: (event: Event) => Promise<unknown>,
  errorHandler?: (error: Error) => unknown,
): (event: Event) => void {
  return (event: Event) => {
    listener(event).catch(errorHandler ?? ((error) => console.error(error)));
  };
}
