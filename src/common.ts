import { message } from '@tauri-apps/api/dialog';
import { InvokeArgs, invoke as tauriInvoke } from '@tauri-apps/api/tauri';

export async function invoke<T>(
  cmd: string,
  args?: InvokeArgs,
  updateStatus?: () => Promise<void> | void
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
