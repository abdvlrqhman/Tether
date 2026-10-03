import { useEffect, useState } from 'react';
import {
  Copy,
  Check,
  ArrowRight,
  Stop,
  ShieldCheck,
  Folder,
  Globe,
  ArrowsClockwise,
  UserCircle,
  Plug,
} from '@phosphor-icons/react';
import type { Host } from '../contracts';
import { native } from '../services/desktop';
type Run = (name: string, args?: Record<string, unknown>) => Promise<boolean>;
export function HostPanel({
  host,
  home,
  busy,
  run,
  onError,
}: {
  host: Host;
  home: string;
  busy: boolean;
  run: Run;
  onError: (e: string) => void;
}) {
  const [cwd, setCwd] = useState(''),
    [online, setOnline] = useState(true),
    [minutes, setMinutes] = useState(30),
    [copied, setCopied] = useState(false),
    [consent, setConsent] = useState(false);
  useEffect(() => {
    if (home) setCwd((old) => old || home);
  }, [home]);
  const ready = host.running && ['local', 'online'].includes(host.tunnel);
  const invitation =
    host.invite && ready ? JSON.stringify({ version: 1, url: host.url, invite: host.invite }) : '';
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(invitation);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      onError('Clipboard unavailable. Select and copy the invitation below.');
    }
  };
  return (
    <>
      <section className="section-heading">
        <div className="eyebrow">HOST WORKSPACE</div>
        <h1>Bring debugging closer.</h1>
        <p>Give a developer or an agent a temporary terminal on this device.</p>
      </section>
      {!host.running ? (
        <section className="setup-panel">
          <div className="panel-heading">
            <h2>Share this device</h2>
            <span className="tag">Host approval required</span>
          </div>
          <label className="field-label" htmlFor="cwd">
            Starting folder
          </label>
          <div className="input-icon">
            <Folder size={19} />
            <input
              id="cwd"
              value={cwd}
              placeholder="Absolute path to your project"
              onChange={(e) => setCwd(e.target.value)}
            />
          </div>
          <p className="field-note">The shell starts here. This folder is not a sandbox.</p>
          <fieldset className="route-options">
            <legend>Connection route</legend>
            <label className={online ? 'route selected' : 'route'}>
              <input type="radio" name="route" checked={online} onChange={() => setOnline(true)} />
              <Globe size={22} />
              <span>
                <strong>Internet</strong>
                <small>Temporary TryCloudflare tunnel</small>
              </span>
              {online && <Check size={17} />}
            </label>
            <label className={!online ? 'route selected' : 'route'}>
              <input
                type="radio"
                name="route"
                checked={!online}
                onChange={() => setOnline(false)}
              />
              <Plug size={22} />
              <span>
                <strong>Local only</strong>
                <small>Loopback for testing on this device</small>
              </span>
              {!online && <Check size={17} />}
            </label>
          </fieldset>
          <div className="access-note">
            <ShieldCheck size={22} />
            <div>
              <strong>You stay in control.</strong>
              <p>
                Approve each operator locally. End access at any time. Nothing runs before you
                approve.
              </p>
            </div>
          </div>
          <label className="consent">
            <input
              type="checkbox"
              checked={consent}
              onChange={(e) => setConsent(e.target.checked)}
            />
            <span>
              I understand approved operators can run commands and read or change files with my user
              account’s permissions.
            </span>
          </label>
          <div className="panel-bottom">
            <span>No account. No permanent connection.</span>
            <button
              className="primary"
              disabled={busy || !consent || !cwd.trim()}
              onClick={() => void run('start_host', { workingDirectory: cwd, online })}
            >
              {busy ? 'Starting…' : 'Start sharing'}
              <ArrowRight size={18} />
            </button>
          </div>
        </section>
      ) : (
        <>
          <section className="setup-panel">
            <div className="panel-heading">
              <h2>
                {host.session
                  ? 'A session is active'
                  : ready
                    ? 'Your device is ready to pair'
                    : 'Connecting this device'}
              </h2>
              <span className={'tag ' + (ready ? 'live' : '')}>
                {host.tunnel === 'starting'
                  ? 'Opening tunnel…'
                  : host.tunnel === 'online'
                    ? 'Tunnel online'
                    : host.tunnel === 'failed'
                      ? 'Tunnel failed'
                      : 'Local connection'}
              </span>
            </div>
            <div className="endpoint">
              <Globe size={19} />
              <code>{host.url}</code>
            </div>
            {host.invite ? (
              <>
                <p className="field-note">
                  Send this invitation privately. It expires after 10 minutes and is consumed when
                  you approve an operator.
                </p>
                <textarea
                  className="invitation-output"
                  aria-label="Invitation JSON"
                  readOnly
                  value={invitation}
                  placeholder="The invitation appears when the connection is ready."
                />
                <div className="button-row">
                  <button className="primary" disabled={!ready || busy} onClick={() => void copy()}>
                    {copied ? <Check size={18} /> : <Copy size={18} />}{' '}
                    {copied ? 'Copied invitation' : 'Copy invitation'}
                  </button>
                  <button
                    className="quiet"
                    disabled={busy}
                    onClick={() => void run('renew_invite')}
                  >
                    <ArrowsClockwise size={17} />
                    New invitation
                  </button>
                </div>
              </>
            ) : !host.session ? (
              <>
                <p className="field-note">Create a fresh invitation to pair another operator.</p>
                <button
                  className="primary"
                  disabled={busy}
                  onClick={() => void run('renew_invite')}
                >
                  New invitation
                  <ArrowRight size={17} />
                </button>
              </>
            ) : null}
            <div className="panel-bottom">
              <span>{host.working_directory}</span>
              <button className="danger" disabled={busy} onClick={() => void run('stop_host')}>
                <Stop size={16} weight="fill" />
                Stop sharing
              </button>
            </div>
          </section>
          {host.pending.length > 0 && (
            <section className="approval-panel">
              <div className="panel-heading">
                <h2>Approve an operator</h2>
                <label className="duration">
                  Access for
                  <select
                    aria-label="Session duration"
                    value={minutes}
                    onChange={(e) => setMinutes(Number(e.target.value))}
                  >
                    {[15, 30, 60, 120].map((m) => (
                      <option key={m} value={m}>
                        {m} minutes
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              <p>
                This grants full shell access under your account. The operator name is
                self-reported; verify the request with the person you invited.
              </p>
              {host.pending.map((p) => (
                <div className="approval-row" key={p.id}>
                  <UserCircle size={32} weight="light" />
                  <div>
                    <strong>{p.operator}</strong>
                    <small>Request {p.id.slice(0, 8)} · Waiting for your approval</small>
                  </div>
                  <button
                    className="quiet"
                    disabled={busy}
                    onClick={() => void run('deny_pair', { id: p.id })}
                  >
                    Deny
                  </button>
                  <button
                    className="primary"
                    disabled={busy}
                    onClick={() => void run('approve_pair', { id: p.id, minutes })}
                  >
                    Approve access
                  </button>
                </div>
              ))}
            </section>
          )}
          {host.session && (
            <section className="approval-panel">
              <div className="approval-row">
                <UserCircle size={32} />
                <div>
                  <strong>{host.session.operator}</strong>
                  <small>
                    Access expires {new Date(host.session.expires_at * 1000).toLocaleTimeString()}
                  </small>
                </div>
                <button
                  className="danger"
                  disabled={busy}
                  onClick={() => void run('revoke_session')}
                >
                  End access
                </button>
              </div>
            </section>
          )}
        </>
      )}
      {!native && (
        <div className="preview-note">
          Browser preview · Device access works in the Tauri desktop app.
        </div>
      )}
      <div className="flow-strip">
        <div>
          <span>1</span>
          <strong>Share an invitation</strong>
        </div>
        <ArrowRight />
        <div>
          <span>2</span>
          <strong>Approve the operator</strong>
        </div>
        <ArrowRight />
        <div>
          <span>3</span>
          <strong>Debug together</strong>
        </div>
      </div>
    </>
  );
}
