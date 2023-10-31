import { ask } from '@tauri-apps/api/dialog';
import { commands, events, type PortStateEvent } from './commands';
import { asyncListener, displayError, toggleControls } from './common';

function setupDirectionButton(button: HTMLButtonElement): void {
  const direction = button.dataset['direction'];

  if (!direction) {
    return;
  }

  const isZoom = direction === 'in' || direction === 'out';
  const { command, stop } = isZoom
    ? { command: (direction: string) => commands.zoom(direction), stop: () => commands.stopZoom() }
    : {
        command: (direction: string) => commands.moveCamera(direction),
        stop: () => commands.stopMove(),
      };

  button.addEventListener(
    'pointerdown',
    asyncListener(async (event) => {
      await command(direction);

      const controller = new AbortController();

      button.addEventListener(
        'pointerup',
        asyncListener(async (event) => {
          try {
            await stop();
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

async function populatePorts(portSelect: HTMLSelectElement): Promise<void> {
  while (portSelect.lastChild) {
    portSelect.removeChild(portSelect.lastChild);
  }

  portSelect.appendChild(document.createElement('option'));

  let ports: string[];

  try {
    const result = await commands.getPorts();

    if (result.status === 'error') {
      throw new Error(result.error);
    }

    ports = result.data;
  } catch (e) {
    await displayError(e);
    return;
  }

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
    try {
      await commands.setPreset(preset, presetName);
    } catch (e) {
      await displayError(e);
    }
  }
}

function onStateChange({ port }: PortStateEvent) {
  console.log(port);
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

window.addEventListener(
  'DOMContentLoaded',
  asyncListener(async (): Promise<void> => {
    const port = document.querySelector<HTMLSelectElement>('#ports');

    if (!port) {
      return;
    }

    await populatePorts(port);

    port.addEventListener(
      'change',
      asyncListener(async (event) => {
        const target = event.target as HTMLSelectElement;
        const value = target.options[target.selectedIndex]?.value ?? null;

        console.log(value);
        await commands.setPort(value === '' ? null : value);
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

    await events.portStateEvent.listen(({ payload }) => onStateChange(payload));

    await commands.ready();
  }),
);
