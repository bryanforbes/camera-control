import { listen } from '@tauri-apps/api/event';
// import * as store from './store';
import { asyncListener, toggleControls, CameraState } from './common';
import { invoke } from '@tauri-apps/api';
import * as tauriCommands from './commands';

const commands = {
  power: 'camera_power',
  autofocus: 'autofocus',
} as const;

async function handleToggle(event: Event): Promise<void> {
  const toggle = (event.target as HTMLElement).closest<HTMLInputElement>(
    'input[type="checkbox"].toggle[data-function]',
  );

  if (!toggle) {
    return;
  }

  const func = toggle.dataset['function'] as 'power' | 'autofocus' | undefined;
  if (!func) {
    return;
  }

  await invoke(commands[func], { [func]: toggle.checked });
}

async function handleControl(event: MouseEvent): Promise<void> {
  const button = (event.target as HTMLElement).closest<HTMLButtonElement>('button[data-function]');

  if (!button) {
    return;
  }

  const functionData = button.dataset['function'];
  if (!functionData) {
    return;
  }

  const [func, state] = functionData.split('-') as ['power' | 'autofocus', string];

  await invoke(commands[func], { state: state == 'on' });
}

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

  await tauriCommands.goToPreset(preset, presetName);
}

function onStateChange({ status, port, power, autofocus }: CameraState) {
  toggleControls('.power', Boolean(port));
  toggleControls('.autofocus', power);
  toggleControls('.presets', power);

  console.log(status);
  const statusElement = document.querySelector('.status');
  if (statusElement) {
    statusElement.textContent = status;
  }

  const powerElement = document.querySelector<HTMLInputElement>('.power .toggle');
  if (powerElement) {
    powerElement.checked = power;
  }

  const autofocusElement = document.querySelector<HTMLInputElement>('.autofocus .toggle');
  if (autofocusElement) {
    autofocusElement.checked = autofocus;
  }
}

window.addEventListener(
  'DOMContentLoaded',
  asyncListener(async (): Promise<void> => {
    document.querySelector('.settings')?.addEventListener(
      'click',
      asyncListener(() => tauriCommands.openSettings()),
    );

    document.querySelector('.controls')?.addEventListener(
      'click',
      asyncListener((event) => handleControl(event as MouseEvent)),
    );

    document.querySelector('.toggles')?.addEventListener('change', asyncListener(handleToggle));

    document.querySelector('.presets')?.addEventListener(
      'click',
      asyncListener((event) => goToPreset(event as MouseEvent)),
    );

    onStateChange(await invoke('get_state'));
    await listen<CameraState>('camera-state', ({ payload }) => onStateChange(payload));
  }),
);
