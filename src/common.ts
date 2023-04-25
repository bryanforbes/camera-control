import { message } from '@tauri-apps/api/dialog';

export async function displayError(e: unknown): Promise<void> {
  await message(`${e}`, {
    title: 'Error',
    type: 'error',
  });
}

export function toggleNoPort(selector: string, noPort: boolean): void {
  document.querySelector(selector)?.classList.toggle('no-port', noPort);
}
