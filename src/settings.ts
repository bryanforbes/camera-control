import * as store from './store';
import { displayError, invoke, toggleControls } from './common';
import { ask } from '@tauri-apps/api/dialog';
import { WebviewWindow } from '@tauri-apps/api/window';

async function setStatus(status: string): Promise<void> {
  await WebviewWindow.getByLabel('main')?.emit('status', status);
}

function setupDirectionButton(button: HTMLButtonElement): void {
  const direction = button.dataset['direction'];
  const isZoom = direction === 'in' || direction === 'out';
  const command = isZoom ? 'zoom' : 'move_camera';
  const status = `${isZoom ? 'Zooming' : 'Moving'} ${direction}`;
  const statusSetter = () => setStatus(status);
  const stopStatus = `Done ${isZoom ? 'zooming' : 'moving'}`;
  const stopStatusSetter = () => setStatus(stopStatus);

  button.addEventListener('pointerdown', async (event) => {
    try {
      await invoke(command, { direction }, statusSetter);
    } catch (e) {
      await displayError(e);
      return;
    }

    const controller = new AbortController();

    button.addEventListener(
      'pointerup',
      async (event) => {
        try {
          await invoke('stop', undefined, stopStatusSetter);
        } catch (e) {
          await displayError(e);
        } finally {
          controller.abort();
          button.releasePointerCapture(event.pointerId);
        }
      },
      { signal: controller.signal }
    );

    button.setPointerCapture(event.pointerId);
  });
}

let portSelect: HTMLSelectElement;
let populating = false;

async function populatePorts(): Promise<void> {
  populating = true;

  while (portSelect.lastChild) {
    portSelect.removeChild(portSelect.lastChild);
  }

  portSelect.appendChild(document.createElement('option'));

  const savedPort = await store.getPort();
  let ports: string[];

  try {
    ports = await invoke<string[]>('get_ports');
  } catch (e) {
    await displayError(e);
    populating = false;
    return;
  }

  for (const port of ports) {
    const portOption = document.createElement('option');

    if (port === savedPort) {
      portOption.selected = true;
    }
    portOption.value = port;

    portOption.appendChild(document.createTextNode(port));
    portSelect.appendChild(portOption);
  }

  populating = false;
}

async function confirmSetPreset(event: MouseEvent): Promise<void> {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-preset]');

  if (!button) {
    return;
  }

  const preset = parseInt(button.dataset['preset']!, 10);
  const presetName = button.dataset['presetName']!;

  const confirmed = await ask(`Are you sure you want to set ${presetName}?`, { type: 'warning' });

  if (confirmed) {
    try {
      await invoke('set_preset', { preset }, () => setStatus(`${presetName} preset set`));
    } catch (e) {
      await displayError(e);
    }
  }
}

window.addEventListener('DOMContentLoaded', async (): Promise<void> => {
  portSelect = document.querySelector('#ports')!;

  await populatePorts();

  store.onPortChange((value) => toggleControls('.controls', Boolean(value)));

  portSelect.addEventListener('change', async (event) => {
    if (populating) {
      return;
    }

    const target = event.target as HTMLSelectElement;

    await store.setPort(target.options[target.selectedIndex]?.value || null);
    await store.save();
  });

  for (const button of document.querySelectorAll<HTMLButtonElement>('.controls [data-direction]')) {
    setupDirectionButton(button);
  }

  document
    .querySelector('.presets')
    ?.addEventListener('click', (event) => confirmSetPreset(event as MouseEvent));
});
