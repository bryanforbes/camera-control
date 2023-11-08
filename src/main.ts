import {
  createAddAsyncEventListener,
  invoke,
  listen,
  toggleControls,
  type PortState,
} from './common';

function setStatus(status: string) {
  const statusNode = document.querySelector<HTMLParagraphElement>('.status');

  if (!statusNode) {
    return;
  }

  statusNode.innerText = status;
}

const addAsyncEventListener = createAddAsyncEventListener((error) => setStatus(`Error: ${error}`));

async function goToPreset(event: MouseEvent): Promise<void> {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-preset]');

  if (!button) {
    return;
  }

  const preset = parseInt(button.dataset['preset'] ?? '', 10);
  const presetName = button.dataset['presetName'] ?? 'Unknown';

  if (isNaN(preset)) {
    return;
  }

  await invoke('go_to_preset', { preset });
  setStatus(presetName);
}

function onStateChange({ port }: PortState) {
  toggleControls('.controls', Boolean(port));
  toggleControls('.presets', Boolean(port));

  setStatus(port ? 'Connected' : 'Disconnected');
}

addAsyncEventListener(window, 'DOMContentLoaded', async (): Promise<void> => {
  addAsyncEventListener(document.querySelector<HTMLButtonElement>('.settings'), 'click', () =>
    invoke('open_settings'),
  );

  addAsyncEventListener(document.querySelector<HTMLElement>('.presets'), 'click', goToPreset);

  await listen('port-state', ({ payload }) => onStateChange(payload));
  await listen('status', ({ payload }) => setStatus(payload));

  await invoke('ready');
});
