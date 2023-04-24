import { invoke } from '@tauri-apps/api/tauri';
import * as store from './store';

let portSelect: HTMLSelectElement;
let populating = false;

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
  document.querySelector('.arrows')?.classList.toggle('no-port', disabled);
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

    await store.setPort(target.options[target.selectedIndex]?.value ?? '');
    await store.save();
  });
});
