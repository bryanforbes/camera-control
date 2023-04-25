import { Store } from 'tauri-plugin-store-api';
import type { UnlistenFn } from '@tauri-apps/api/event';

const store = new Store('config.json');

export async function onPortChange(callback: (port: string | null) => void): Promise<UnlistenFn> {
  callback(await getPort());
  return store.onKeyChange('port', callback);
}

export async function getPort(): Promise<string | null> {
  return store.get('port');
}

export async function setPort(port: string | null): Promise<void> {
  return store.set('port', port);
}

export async function save(): Promise<void> {
  return store.save();
}
