import { listen } from '@tauri-apps/api/event';
import { commands, events, type PortStateEvent } from './bindings';
import { createAddAsyncEventListener, toggleControls } from './common';

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

  await commands.goToPreset(preset);
  setStatus(presetName);
}

function onStateChange({ port }: PortStateEvent) {
  toggleControls('.controls', Boolean(port));
  toggleControls('.presets', Boolean(port));

  setStatus(port ? 'Connected' : 'Disconnected');
}

addAsyncEventListener(window, 'DOMContentLoaded', async (): Promise<void> => {
  addAsyncEventListener(document.querySelector<HTMLButtonElement>('.settings'), 'click', () =>
    commands.openSettings(),
  );

  addAsyncEventListener(document.querySelector<HTMLElement>('.presets'), 'click', goToPreset);

  await events.portStateEvent.listen(({ payload }) => onStateChange(payload));
  await listen<string>('status', ({ payload }) => setStatus(payload));

  await commands.ready();
});
