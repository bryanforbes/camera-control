import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/tauri';
import { WebviewWindow } from '@tauri-apps/api/window';
import * as store from './store';
import { displayError, toggleControls } from './common';

function updateStatus(status: string): void {
  const element = document.querySelector('.status');

  if (element) {
    element.textContent = status;
  }
}

async function goToPreset(event: MouseEvent): Promise<void> {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-preset]');

  if (!button) {
    return;
  }

  const preset = parseInt(button.dataset['preset']!, 10);
  const presetName = button.dataset['presetName']!;

  try {
    await invoke('go_to_preset', { preset });
    updateStatus(presetName);
  } catch (e) {
    await displayError(e);
  }
}

async function openSettings(): Promise<void> {
  let settingsWindow = WebviewWindow.getByLabel('settings');

  if (settingsWindow) {
    await settingsWindow.setFocus();
  } else {
    settingsWindow = new WebviewWindow('settings', {
      url: 'settings.html',
      title: 'Camera Control Settings',
      resizable: false,
      acceptFirstMouse: true,
      width: 600,
      height: 480,
    });
  }
}

window.addEventListener('DOMContentLoaded', async (): Promise<void> => {
  store.onPortChange((value) => {
    emit('port-changed', value);
    toggleControls('.presets', Boolean(value));
  });
  document
    .querySelector('.presets')
    ?.addEventListener('click', (event) => goToPreset(event as MouseEvent));

  document.querySelector('.settings')?.addEventListener('click', () => openSettings());
  listen('open-settings', () => openSettings());
  listen('status', (event) => updateStatus(event.payload as string));
});
