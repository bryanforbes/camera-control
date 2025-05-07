import type { UnlistenFn } from '@tauri-apps/api/event';
import { Store } from '@tauri-apps/plugin-store';

const store = await Store.load('config.json');

export async function onPortChange(
  callback: (port: string | null | undefined) => void,
): Promise<UnlistenFn> {
  callback(await getPort());
  return store.onKeyChange('port', callback);
}

export async function getPort(): Promise<string | null | undefined> {
  return store.get('port');
}

export async function setPort(port: string | null): Promise<void> {
  return store.set('port', port);
}

export async function save(): Promise<void> {
  return store.save();
}
