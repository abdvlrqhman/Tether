import { useCallback, useEffect, useRef, useState } from 'react';
import type { Overview } from '../contracts';
import { command, empty, native } from '../services/desktop';
export function useDesktop() {
  const [data, setData] = useState<Overview>(empty),
    [busy, setBusy] = useState(false),
    [error, setError] = useState('');
  const active = useRef(true);
  const refresh = useCallback(async () => {
    if (!native) return;
    const snapshot = await command<Overview>('overview');
    if (active.current) setData(snapshot);
  }, []);
  useEffect(() => {
    active.current = true;
    let timer: ReturnType<typeof setTimeout>;
    let poll = true;
    const tick = async () => {
      try {
        await command('poll_remote');
        await refresh();
      } catch (e) {
        if (poll) setError(String(e));
      }
      if (poll) timer = setTimeout(tick, 1000);
    };
    if (native) void tick();
    return () => {
      active.current = false;
      poll = false;
      clearTimeout(timer);
    };
  }, [refresh]);
  const run = useCallback(
    async (name: string, args?: Record<string, unknown>) => {
      setBusy(true);
      setError('');
      try {
        await command(name, args);
        await refresh();
        return true;
      } catch (e) {
        setError(String(e));
        return false;
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );
  return { data, busy, error, setError, run };
}
