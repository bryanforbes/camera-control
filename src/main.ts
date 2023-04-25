import { emit } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/tauri';
import { WebviewWindow } from '@tauri-apps/api/window';
import * as store from './store';

async function restorePreset(event: MouseEvent) {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-preset]');

  if (!button) {
    return;
  }

  const preset = parseInt(button.dataset['preset'] ?? '1', 10);

  await invoke('restore_preset', { preset });
}

async function openSettings() {
  let settingsWindow = WebviewWindow.getByLabel('settings');

  if (settingsWindow) {
    await settingsWindow.setFocus();
  } else {
    settingsWindow = new WebviewWindow('settings', {
      url: 'settings.html',
      title: 'Camera Control Settings',
      resizable: false,
    });
  }
}

function setDisabledState(disabled: boolean) {
  document.querySelector('.presets')?.classList.toggle('no-port', disabled);
}

window.addEventListener('DOMContentLoaded', async () => {
  store.onPortChange((value) => {
    emit('port-changed', value);
    setDisabledState(!value);
  });
  document
    .querySelector('.presets')
    ?.addEventListener('click', (event) => restorePreset(event as MouseEvent));

  document.querySelector('.settings')?.addEventListener('click', () => openSettings());
});
