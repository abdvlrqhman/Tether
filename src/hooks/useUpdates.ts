import { useEffect, useRef, useState } from 'react';
import type { Update } from '@tauri-apps/plugin-updater';
import { checkForUpdate, installUpdate, restartAfterUpdate } from '../services/updates';

export function useUpdates() {
  const [state, setState] = useState<
    | 'idle'
    | 'checking'
    | 'current'
    | 'available'
    | 'downloading'
    | 'installing'
    | 'installed'
    | 'error'
  >('idle');
  const [release, setRelease] = useState<{ version: string; notes: string } | null>(null);
  const [error, setError] = useState('');
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);
  const [progress, setProgress] = useState({ downloaded: 0, total: 0 });
  const update = useRef<Update | null>(null),
    locked = useRef(false),
    mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      void update.current?.close().catch(() => {});
    };
  }, []);
  async function checkNow() {
    if (locked.current) return;
    locked.current = true;
    setError('');
    setState('checking');
    try {
      await update.current?.close();
      update.current = null;
      setRelease(null);
      const next = await checkForUpdate();
      if (!mounted.current) {
        await next?.close();
        return;
      }
      update.current = next;
      setRelease(
        next
          ? { version: next.version, notes: next.body || 'See the GitHub release for details.' }
          : null,
      );
      setCheckedAt(new Date());
      setState(next ? 'available' : 'current');
    } catch (e) {
      const failed = update.current;
      update.current = null;
      await failed?.close().catch(() => {});
      if (mounted.current) {
        setRelease(null);
        setError(String(e));
        setState('error');
      }
    } finally {
      locked.current = false;
    }
  }
  async function installNow() {
    if (locked.current || !update.current) return;
    locked.current = true;
    setError('');
    setProgress({ downloaded: 0, total: 0 });
    setState('downloading');
    try {
      await installUpdate(update.current, (event) => {
        if (!mounted.current) return;
        if (event.event === 'Started')
          setProgress({ downloaded: 0, total: event.data.contentLength || 0 });
        if (event.event === 'Progress')
          setProgress((p) => ({ ...p, downloaded: p.downloaded + event.data.chunkLength }));
        if (event.event === 'Finished') setState('installing');
      });
      if (mounted.current) setState('installed');
    } catch (e) {
      const failed = update.current;
      update.current = null;
      await failed?.close().catch(() => {});
      if (mounted.current) {
        setRelease(null);
        setError(String(e));
        setState('error');
      }
    } finally {
      locked.current = false;
    }
  }
  async function restart() {
    try {
      await restartAfterUpdate();
    } catch (e) {
      setError(String(e));
    }
  }
  return { state, release, error, progress, checkedAt, checkNow, installNow, restart };
}
