import { emit, listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/tauri';
import { WebviewWindow } from '@tauri-apps/api/window';
import * as store from './store';
import { displayError, toggleNoPort } from './common';

async function goToPreset(event: MouseEvent): Promise<void> {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-preset]');

  if (!button) {
    return;
  }

  const preset = parseInt(button.dataset['preset'] ?? '1', 10);

  try {
    await invoke('go_to_preset', { preset });
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
      acceptFirstMouse: true,
      width: 600,
      height: 550,
    });
  }
}

window.addEventListener('DOMContentLoaded', async (): Promise<void> => {
  store.onPortChange((value) => {
    emit('port-changed', value);
    toggleNoPort('.presets', !value);
  });
  document
    .querySelector('.presets')
    ?.addEventListener('click', (event) => goToPreset(event as MouseEvent));

  document.querySelector('.settings')?.addEventListener('click', () => openSettings());
  listen('open-settings', () => openSettings());
});
