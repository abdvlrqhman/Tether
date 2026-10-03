import { check, type Update, type DownloadEvent } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { command, native } from './desktop';

// The native adapter verifies every downloaded package against our bundled public key.
// Release notes remain plain text; no remote HTML is rendered in the desktop window.
export async function checkForUpdate(): Promise<Update | null> {
  if (!native) throw new Error('Update checks are available in the installed desktop app.');
  return check({ timeout: 20000 });
}
export async function installUpdate(update: Update, onProgress: (e: DownloadEvent) => void) {
  await update.download(onProgress, { timeout: 120000 });
  // Re-check native session state after downloading, then block new sessions until restart.
  await command('begin_update');
  try {
    await update.install();
  } catch (error) {
    await command('cancel_update');
    throw error;
  }
}
export async function restartAfterUpdate() {
  await relaunch();
}
