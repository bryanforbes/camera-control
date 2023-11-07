import { asyncListener, invoke, listen, toggleControls, type PortState } from './common';

function setStatus(status: string) {
  const statusNode = document.querySelector<HTMLParagraphElement>('.status');

  if (!statusNode) {
    return;
  }

  statusNode.innerText = status;
}

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

  try {
    await invoke('go_to_preset', { preset }, () => setStatus(presetName));
  } catch (e) {
    setStatus(`Error: ${e}`);
  }
}

function onStateChange({ port }: PortState) {
  toggleControls('.controls', Boolean(port));
  toggleControls('.presets', Boolean(port));

  setStatus(port ? 'Connected' : 'Disconnected');
}

window.addEventListener(
  'DOMContentLoaded',
  asyncListener(async (): Promise<void> => {
    document.querySelector('.settings')?.addEventListener(
      'click',
      asyncListener(() => invoke('open_settings')),
    );

    document.querySelector('.presets')?.addEventListener(
      'click',
      asyncListener((event) => goToPreset(event as MouseEvent)),
    );

    await listen<PortState>('port-state', onStateChange);
    await listen('status', setStatus);

    await invoke('ready');
  }),
);
