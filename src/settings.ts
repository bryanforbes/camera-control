import { ask } from '@tauri-apps/api/dialog';
import { WebviewWindow } from '@tauri-apps/api/window';
import {
  createAddAsyncEventListener,
  invoke,
  listen,
  toggleControls,
  type PortState,
} from './common';

async function setStatus(status: string): Promise<void> {
  return WebviewWindow.getByLabel('main')?.emit('status', status);
}

const addAsyncEventListener = createAddAsyncEventListener((error) => setStatus(`Error: ${error}`));

function setupDirectionButton(button: HTMLButtonElement): void {
  const direction = button.dataset['direction'];

  if (!direction) {
    return;
  }

  const isZoom = direction === 'in' || direction === 'out';
  const command = isZoom ? 'zoom' : 'move_camera';
  const stopCommand = isZoom ? 'stop_zoom' : 'stop_move';
  const status = `${isZoom ? 'Zooming' : 'Moving'} ${direction}`;
  const stopStatus = `Done ${isZoom ? 'zooming' : 'moving'}`;

  addAsyncEventListener(button, 'pointerdown', async (event) => {
    await invoke(command, { direction });
    await setStatus(status);

    const controller = new AbortController();

    addAsyncEventListener(
      button,
      'pointerup',
      async (event) => {
        try {
          await invoke(stopCommand);
          await setStatus(stopStatus);
        } finally {
          controller.abort();
          button.releasePointerCapture(event.pointerId);
        }
      },
      { signal: controller.signal },
    );

    button.setPointerCapture(event.pointerId);
  });
}

async function populatePorts(portSelect: HTMLSelectElement): Promise<void> {
  while (portSelect.lastChild) {
    portSelect.removeChild(portSelect.lastChild);
  }

  portSelect.appendChild(document.createElement('option'));

  const ports = await invoke('get_ports');

  for (const port of ports) {
    const portOption = document.createElement('option');

    portOption.value = port;

    portOption.appendChild(document.createTextNode(port));
    portSelect.appendChild(portOption);
  }
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
    await invoke('set_preset', { preset });
    await setStatus(`Set ${presetName}`);
  }
}

function onStateChange({ port }: PortState) {
  toggleControls('.controls', Boolean(port));

  const portSelect = document.querySelector<HTMLSelectElement>('#ports');

  if (!portSelect) {
    return;
  }

  for (const option of portSelect.children as Iterable<HTMLOptionElement>) {
    const value = !option.value ? null : option.value;

    if (value === port && !option.selected) {
      option.selected = true;
    } else if (value !== port && option.selected) {
      option.selected = false;
    }
  }
}

addAsyncEventListener(window, 'DOMContentLoaded', async (): Promise<void> => {
  const port = document.querySelector<HTMLSelectElement>('#ports');

  if (!port) {
    return;
  }

  await populatePorts(port);

  addAsyncEventListener(port, 'change', async (event) => {
    const target = event.target as HTMLSelectElement;
    const value = target.options[target.selectedIndex]?.value ?? null;

    console.log(value);
    await invoke('set_port', { portName: value === '' ? null : value });
  });

  for (const button of document.querySelectorAll<HTMLButtonElement>('.controls [data-direction]')) {
    setupDirectionButton(button);
  }

  addAsyncEventListener(document.querySelector<HTMLElement>('.presets'), 'click', confirmSetPreset);

  await listen('port-state', ({ payload }) => onStateChange(payload));

  await invoke('ready');
});
