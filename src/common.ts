import { message } from '@tauri-apps/api/dialog';

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
