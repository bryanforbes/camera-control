import * as store from './store';
import { CameraState, asyncListener, displayError, toggleControls } from './common';
import { ask } from '@tauri-apps/api/dialog';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api';

function setupDirectionButton(button: HTMLButtonElement): void {
  const direction = button.dataset['direction'];
  const isZoom = direction === 'in' || direction === 'out';
  const command = isZoom ? 'zoom' : 'move_camera';
  const stop = isZoom ? 'stop_zoom' : 'stop_move';

  button.addEventListener(
    'pointerdown',
    asyncListener(async (event) => {
      await invoke(command, { direction });

      const controller = new AbortController();

      button.addEventListener(
        'pointerup',
        asyncListener(async (event) => {
          try {
            await invoke(stop);
          } finally {
            controller.abort();
            button.releasePointerCapture(event.pointerId);
          }
        }),
        { signal: controller.signal },
      );

      button.setPointerCapture(event.pointerId);
    }),
  );
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

  const preset = parseInt(button.dataset['preset'] ?? '', 10);
  const presetName = button.dataset['presetName'];

  if (isNaN(preset) || presetName == null) {
    return;
  }

  const confirmed = await ask(`Are you sure you want to set ${presetName}?`, { type: 'warning' });

  if (confirmed) {
    try {
      await invoke('set_preset', { preset });
    } catch (e) {
      await displayError(e);
    }
  }
}

function onStateChange({ power }: CameraState) {
  console.log(power);
  toggleControls('.controls', power);
}

window.addEventListener(
  'DOMContentLoaded',
  asyncListener(async (): Promise<void> => {
    const port = document.querySelector<HTMLSelectElement>('#ports');

    if (!port) {
      return;
    }

    portSelect = port;

    await populatePorts();

    portSelect.addEventListener(
      'change',
      asyncListener(async (event) => {
        if (populating) {
          return;
        }

        const target = event.target as HTMLSelectElement;

        console.log(target.options[target.selectedIndex]?.value ?? null);
        await invoke('set_port', {
          port: target.options[target.selectedIndex]?.value ?? null,
        });
      }),
    );

    for (const button of document.querySelectorAll<HTMLButtonElement>(
      '.controls [data-direction]',
    )) {
      setupDirectionButton(button);
    }

    document.querySelector('.presets')?.addEventListener(
      'click',
      asyncListener((event) => confirmSetPreset(event as MouseEvent)),
    );

    onStateChange(await invoke<CameraState>('get_state'));
    await listen<CameraState>('camera-state', (event) => onStateChange(event.payload));
  }),
);
