import { invoke } from '@tauri-apps/api/tauri';
import * as store from './store';

let portSelect: HTMLSelectElement;
let populating = false;

function setupDirectionButton(button: HTMLButtonElement) {
  const direction = button.dataset['direction'];
  const command = direction === 'in' || direction === 'out' ? 'zoom' : 'move_camera';

  button.addEventListener('pointerdown', async (event) => {
    const controller = new AbortController();

    button.addEventListener(
      'pointerup',
      async (event) => {
        await invoke('stop');

        controller.abort();
        button.releasePointerCapture(event.pointerId);
      },
      { signal: controller.signal }
    );

    button.setPointerCapture(event.pointerId);

    await invoke(command, { direction });
  });
}

async function populatePorts() {
  populating = true;

  while (portSelect.lastChild) {
    portSelect.removeChild(portSelect.lastChild);
  }

  portSelect.appendChild(document.createElement('option'));

  const savedPort = await store.getPort();

  for (const port of await invoke<string[]>('get_ports')) {
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

function setDisabledState(disabled: boolean) {
  document.querySelector('.controls')?.classList.toggle('no-port', disabled);
}

window.addEventListener('DOMContentLoaded', async () => {
  portSelect = document.querySelector('#ports')!;

  await populatePorts();

  store.onPortChange((value) => setDisabledState(!value));

  portSelect.addEventListener('change', async (event) => {
    if (populating) {
      return;
    }

    const target = event.target as HTMLSelectElement;

    await store.setPort(target.options[target.selectedIndex]?.value || null);
    await store.save();
  });

  document
    .querySelectorAll<HTMLButtonElement>('.controls [data-direction]')
    .forEach((button) => setupDirectionButton(button));
});
