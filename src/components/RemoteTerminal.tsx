import { useEffect, useRef, useState } from 'react';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { listen } from '@tauri-apps/api/event';
import { command, native } from '../services/desktop';
import { TerminalWindow, ArrowClockwise } from '@phosphor-icons/react';
import '@xterm/xterm/css/xterm.css';
interface Frame {
  type: string;
  data?: string;
  message?: string;
}
export function RemoteTerminal({
  connected,
  onError,
}: {
  connected: boolean;
  onError: (error: string) => void;
}) {
  const container = useRef<HTMLDivElement>(null),
    terminal = useRef<Terminal | null>(null),
    fit = useRef<FitAddon | null>(null),
    opened = useRef(false);
  const [status, setStatus] = useState('idle');
  const errorRef = useRef(onError);
  errorRef.current = onError;
  const channel = useRef('');
  useEffect(() => {
    let cleanup = () => {};
    const frame = requestAnimationFrame(() => {
      if (!container.current) return;
      const term = new Terminal({
        cursorBlink: true,
        fontFamily: '"Cascadia Code", "SFMono-Regular", Consolas, monospace',
        fontSize: 13,
        lineHeight: 1.45,
        scrollback: 5000,
        allowProposedApi: false,
        theme: {
          background: '#101315',
          foreground: '#d9dfdf',
          cursor: '#9ce2ba',
          selectionBackground: '#324b3c',
          green: '#9ce2ba',
        },
      });
      const addon = new FitAddon();
      term.loadAddon(addon);
      term.open(container.current);
      terminal.current = term;
      fit.current = addon;
      addon.fit();
      const send = (value: object) => {
        void command('terminal_send', { message: JSON.stringify(value) }).catch((e) =>
          errorRef.current(String(e)),
        );
      };
      const data = term.onData((text) => {
        if (opened.current) {
          const bytes = new TextEncoder().encode(text);
          send({
            type: 'input',
            data: btoa(Array.from(bytes, (b) => String.fromCharCode(b)).join('')),
          });
        }
      });
      const resize = term.onResize(({ cols, rows }) => {
        if (opened.current) send({ type: 'resize', cols, rows });
      });
      const observer = new ResizeObserver(() => addon.fit());
      observer.observe(container.current);
      let disposed = false,
        unlisten: (() => void) | undefined;
      if (native)
        void listen<{ terminal_id: string; frame: Frame }>('terminal-message', (event) => {
          if (disposed || event.payload.terminal_id !== channel.current) return;
          const frame = event.payload.frame;
          if (frame.type === 'output' && frame.data) {
            try {
              const bytes = Uint8Array.from(atob(frame.data), (c) => c.charCodeAt(0));
              term.write(bytes);
            } catch {
              errorRef.current('Host returned invalid terminal output.');
            }
          }
          if (frame.type === 'error') errorRef.current(frame.message ?? 'Terminal error');
          if (frame.type === 'closed') {
            opened.current = false;
            setStatus('closed');
            term.writeln('\r\n\x1b[90mSession terminal closed.\x1b[0m');
          }
        })
          .then((fn) => {
            if (disposed) fn();
            else unlisten = fn;
          })
          .catch((e) => errorRef.current(String(e)));
      cleanup = () => {
        disposed = true;
        opened.current = false;
        unlisten?.();
        observer.disconnect();
        data.dispose();
        resize.dispose();
        term.dispose();
        terminal.current = null;
        void command('close_terminal').catch(() => {});
      };
    });
    return () => {
      cancelAnimationFrame(frame);
      cleanup();
    };
  }, []);
  useEffect(() => {
    if (!connected) {
      opened.current = false;
      setStatus('idle');
    }
  }, [connected]);
  async function open() {
    channel.current = crypto.randomUUID();
    opened.current = true;
    setStatus('opening');
    try {
      await command('open_terminal', { terminalId: channel.current });
      opened.current = true;
      setStatus('open');
      fit.current?.fit();
      if (terminal.current) {
        await command('terminal_send', {
          message: JSON.stringify({
            type: 'resize',
            cols: terminal.current.cols,
            rows: terminal.current.rows,
          }),
        });
        terminal.current.focus();
      }
    } catch (e) {
      opened.current = false;
      setStatus('closed');
      onError(String(e));
    }
  }
  return (
    <section className="terminal-panel">
      <div className="terminal-top">
        <span>
          <TerminalWindow size={17} /> Remote terminal
        </span>
        <span className="terminal-status">
          {status === 'open'
            ? 'Shell connected'
            : status === 'opening'
              ? 'Opening shell…'
              : 'No shell attached'}
        </span>
        <button
          className="quiet small"
          disabled={!connected || status === 'opening' || status === 'open'}
          onClick={() => void open()}
        >
          <ArrowClockwise size={15} />
          {status === 'closed' ? 'Reconnect shell' : 'Open shell'}
        </button>
      </div>
      <div className="terminal-body">
        <div className="xterm-mount" ref={container} />
        {status === 'idle' && (
          <div className="terminal-empty">
            <TerminalWindow size={36} weight="light" />
            <h3>No device connected</h3>
            <p>
              Connect to a device, get the host’s approval,
              <br />
              then open an interactive shell.
            </p>
          </div>
        )}
      </div>
      <div className="terminal-footer">
        <span>UTF-8</span>
        <span>Native PTY</span>
        <span>Input and output are not recorded</span>
      </div>
    </section>
  );
}
