import { commands, events, type PortStateEvent } from './commands';
import { asyncListener, toggleControls } from './common';

async function goToPreset(event: MouseEvent): Promise<void> {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-preset]');

  if (!button) {
    return;
  }

  const preset = parseInt(button.dataset['preset'] ?? '', 10);
  const presetName = button.dataset['presetName'];

  if (isNaN(preset) || presetName == null) {
    return;
  }

  await commands.goToPreset(preset, presetName);
}

function onStateChange({ status, port }: PortStateEvent) {
  toggleControls('.power', Boolean(port));
  toggleControls('.presets', Boolean(port));

  console.log(status);

  const statusElement = document.querySelector('.status');
  if (statusElement) {
    statusElement.textContent = status;
  }
}

window.addEventListener(
  'DOMContentLoaded',
  asyncListener(async (): Promise<void> => {
    document.querySelector('.settings')?.addEventListener(
      'click',
      asyncListener(() => commands.openSettings()),
    );

    document.querySelector('.presets')?.addEventListener(
      'click',
      asyncListener((event) => goToPreset(event as MouseEvent)),
    );

    await events.portStateEvent.listen(({ payload }) => onStateChange(payload));

    await commands.ready();
  }),
);
