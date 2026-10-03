import { useState } from 'react';
import { ArrowRight, Link, ShieldCheck, SpinnerGap, Stop } from '@phosphor-icons/react';
import type { Remote } from '../contracts';
import { RemoteTerminal } from './RemoteTerminal';
export function OperatorPanel({
  remote,
  busy,
  run,
  onError,
}: {
  remote: Remote;
  busy: boolean;
  run: (name: string, args?: Record<string, unknown>) => Promise<boolean>;
  onError: (e: string) => void;
}) {
  const [invitation, setInvitation] = useState(''),
    [operator, setOperator] = useState('Spacie developer');
  const connected = remote.status === 'connected',
    pending = remote.status === 'pending';
  return (
    <>
      <section className="section-heading">
        <h1>Connect to a workspace</h1>
        <p>Paste the host’s invitation. Your terminal opens after they approve access.</p>
      </section>
      <section className="connect-panel">
        <div className="panel-heading">
          <h2>
            <Link size={20} />
            Connect to a device
          </h2>
          <span className={'tag ' + (connected ? 'live' : '')}>
            {connected ? 'Connected' : pending ? 'Awaiting approval' : 'Disconnected'}
          </span>
        </div>
        {connected || pending ? (
          <>
            <div className="connection-summary">
              {pending ? <SpinnerGap className="spin" size={24} /> : <ShieldCheck size={24} />}
              <div>
                <strong>
                  {pending
                    ? 'Waiting for the host to approve your request'
                    : `Connected as ${remote.operator}`}
                </strong>
                <small>
                  {remote.url}
                  {remote.os ? ` · ${remote.os}` : ''}
                  {remote.expires_at
                    ? ` · Ends ${new Date(remote.expires_at * 1000).toLocaleTimeString()}`
                    : ''}
                </small>
              </div>
              <button
                className="quiet"
                disabled={busy}
                onClick={() => void run('disconnect_remote')}
              >
                <Stop size={16} />
                Disconnect
              </button>
            </div>
            <p className="field-note">
              Disconnect closes your local terminal. The host can revoke the session from their app.
            </p>
          </>
        ) : (
          <>
            <div className="connect-fields">
              <label>
                <span className="field-label">Invitation from the host</span>
                <textarea
                  value={invitation}
                  onChange={(e) => setInvitation(e.target.value)}
                  placeholder={'{"version":1,"url":"https://…trycloudflare.com","invite":"…"}'}
                  spellCheck={false}
                />
              </label>
              <label>
                <span className="field-label">Your name</span>
                <input
                  value={operator}
                  maxLength={80}
                  onChange={(e) => setOperator(e.target.value)}
                  placeholder="Developer or agent name"
                />
                <small className="field-note">Shown to the host for approval.</small>
              </label>
            </div>
            <div className="panel-bottom">
              <span>Access starts only after the host approves.</span>
              <button
                className="primary"
                disabled={busy || !invitation.trim() || !operator.trim()}
                onClick={() =>
                  void run('connect_remote', { invitation, operator }).then((ok) => {
                    if (ok) setInvitation('');
                  })
                }
              >
                Request access
                <ArrowRight size={18} />
              </button>
            </div>
            {remote.status && (
              <p className="field-note">
                Previous connection: {remote.status}. Ask the host for a new invitation.
              </p>
            )}
          </>
        )}
      </section>
      <RemoteTerminal connected={connected} onError={onError} />
    </>
  );
}
