import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Overview } from '../contracts';
export const native = isTauri();
export const empty: Overview = {
  home: '',
  executable: 'REPLACE_WITH_TETHER_EXECUTABLE',
  audit_path: 'Available in the desktop app',
  host: {
    running: false,
    url: '',
    local_url: '',
    working_directory: '',
    invite: null,
    invite_expires_at: null,
    pending: [],
    session: null,
    audit: [],
    tunnel: 'stopped',
    error: null,
  },
  remote: { status: '', url: '', operator: '', expires_at: null, os: null, error: null },
};
export async function command<T = void>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!native)
    throw new Error(
      'Open the Tether desktop app to share or connect to a device. Run npm run tauri dev.',
    );
  return invoke<T>(name, args);
}
