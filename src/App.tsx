import { useState } from 'react';
import {
  TerminalWindow,
  Desktop,
  ArrowsLeftRight,
  ClockCounterClockwise,
  Robot,
  Info,
  ArrowUpRight,
  ShieldCheck,
  CheckCircle,
  XCircle,
  LinkBreak,
  Code,
  HouseLine,
} from '@phosphor-icons/react';
import { useDesktop } from './hooks/useDesktop';
import type { Mode, Page } from './contracts';
import { HostPanel } from './components/HostPanel';
import { OperatorPanel } from './components/OperatorPanel';
import './styles.css';
import { AgentPanel } from './components/AgentPanel';
import spacieLogo from '../assets/spacie-logo.png';

export default function App() {
  const { data, busy, error, setError, run } = useDesktop();
  const [mode, setMode] = useState<Mode>('host'),
    [page, setPage] = useState<Page>('workspace');
  const host = data.host,
    remote = data.remote,
    active = host.session || remote.status === 'connected';
  const issue = error || host.error || remote.error;
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          onClick={(e) => {
            e.preventDefault();
            setPage('workspace');
          }}
          aria-label="Tether home"
        >
          <div className="brand-mark">
            <TerminalWindow size={25} weight="bold" />
          </div>
          <span>
            Tether
            <small>
              by <img className="spacie-wordmark" src={spacieLogo} alt="Spacie" />
            </small>
          </span>
        </a>
        <div className="sidebar-caption">WORKSPACE</div>
        <nav aria-label="Main navigation">
          <button
            className={page === 'workspace' ? 'nav-item active' : 'nav-item'}
            onClick={() => setPage('workspace')}
          >
            <Desktop size={19} />
            Devices<span className="nav-key">01</span>
          </button>
          <button
            className={page === 'activity' ? 'nav-item active' : 'nav-item'}
            onClick={() => setPage('activity')}
          >
            <ClockCounterClockwise size={19} />
            Activity{host.audit.length > 0 && <span className="nav-key">{host.audit.length}</span>}
          </button>
          <button
            className={page === 'agents' ? 'nav-item active' : 'nav-item'}
            onClick={() => setPage('agents')}
          >
            <Robot size={19} />
            Agent access
          </button>
        </nav>
        <div className="sidebar-bottom">
          <div className="local-device">
            <HouseLine size={21} />
            <div>
              <strong>This device</strong>
              <small>{host.running ? 'Sharing is on' : 'Sharing is off'}</small>
            </div>
            <span className={'status-dot ' + (host.running ? 'on' : '')} />
          </div>
          <button
            className={page === 'about' ? 'nav-item active' : 'nav-item'}
            onClick={() => setPage('about')}
          >
            <Info size={19} />
            About Tether
            <ArrowUpRight size={15} />
          </button>
          <div className="sidebar-version">TETHER / 0.1.0</div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            Workspace<span>/</span>
            <strong>
              {page === 'workspace'
                ? 'Devices'
                : page === 'activity'
                  ? 'Activity'
                  : page === 'agents'
                    ? 'Agent access'
                    : 'About'}
            </strong>
          </div>
          <span className="topbar-status">
            <span className={'status-dot ' + (active ? 'on' : '')} />
            {active ? 'Session active' : 'No active session'}
          </span>
        </header>
        <main>
          <div className="workspace-tools">
            <div className="mode-switch" role="group" aria-label="Device role">
              <button
                className={mode === 'host' ? 'selected' : ''}
                onClick={() => {
                  setMode('host');
                  setPage('workspace');
                }}
              >
                <Desktop size={17} />
                Share this device
              </button>
              <button
                className={mode === 'operator' ? 'selected' : ''}
                onClick={() => {
                  setMode('operator');
                  setPage('workspace');
                }}
              >
                <ArrowsLeftRight size={17} />
                Connect to a device
              </button>
            </div>
            <span className="session-label">SESSION-BASED ACCESS</span>
          </div>
          {issue && (
            <div className="error-banner" role="alert">
              <XCircle size={21} />
              <span>{issue}</span>
              <button
                className="quiet small"
                aria-label="Dismiss error"
                onClick={() => setError('')}
              >
                Dismiss
              </button>
            </div>
          )}
          <div
            className={
              page === 'workspace' && mode === 'host' ? 'content-grid' : 'content-grid full'
            }
          >
            <div className="primary-content">
              {page === 'workspace' &&
                (mode === 'host' ? (
                  <HostPanel
                    host={host}
                    home={data.home}
                    busy={busy}
                    run={run}
                    onError={setError}
                  />
                ) : (
                  <OperatorPanel remote={remote} busy={busy} run={run} onError={setError} />
                ))}
              {page === 'activity' && (
                <>
                  <section className="section-heading">
                    <div className="eyebrow">LOCAL AUDIT LOG</div>
                    <h1>A record of access.</h1>
                    <p>Pairing, approvals, and process lifecycle events on this device.</p>
                  </section>
                  <section className="activity-panel">
                    {host.audit.length === 0 ? (
                      <div className="empty-state">
                        <ClockCounterClockwise size={38} weight="light" />
                        <h2>No activity yet</h2>
                        <p>Share this device to start an audited debugging session.</p>
                      </div>
                    ) : (
                      <ol className="audit-list">
                        {[...host.audit].reverse().map((event, i) => (
                          <li key={`${event.at}-${i}`}>
                            <span className="audit-icon">
                              <ShieldCheck size={18} />
                            </span>
                            <div>
                              <strong>{event.kind.replaceAll('_', ' ')}</strong>
                              <p>{event.detail}</p>
                              <small>{event.actor}</small>
                            </div>
                            <time>{new Date(event.at * 1000).toLocaleTimeString()}</time>
                          </li>
                        ))}
                      </ol>
                    )}
                  </section>
                  <p className="field-note">
                    Persistent JSONL log: <code>{data.audit_path}</code>. This view shows the latest
                    200 events. Command contents and terminal output are not logged.
                  </p>
                </>
              )}
              {page === 'agents' && <AgentPanel executable={data.executable} onError={setError} />}
              {page === 'about' && (
                <>
                  <section className="section-heading">
                    <div className="eyebrow">BUILT BY SPACIE</div>
                    <h1>Debug across the distance.</h1>
                    <p>
                      Tether brings people, agents, and a remote shell into one temporary session.
                    </p>
                  </section>
                  <section className="docs-panel">
                    <h2>
                      <Code size={23} />
                      Small surface. Clear boundaries.
                    </h2>
                    <p>
                      A Tauri 2 desktop shell, a Rust policy and application layer, native process
                      adapters, and a React interface. The same app acts as host and operator on
                      Windows, macOS, and Linux.
                    </p>
                    <h3>Access is powerful</h3>
                    <p>
                      An approved shell can access anything available to the host user, including
                      files outside the starting folder. Tether does not elevate privileges or
                      create an operating-system sandbox.
                    </p>
                    <h3>Temporary internet access</h3>
                    <p>
                      The host starts cloudflared locally. TryCloudflare provides a temporary HTTPS
                      route without an account. Cloudflare terminates TLS; this is transport
                      encryption, not end-to-end encryption.
                    </p>
                    <h3>Shipping this build</h3>
                    <p>
                      This initial release supports terminal debugging. Screen sharing, mouse
                      control, background services, and automatic updates are outside this version.
                      See the README for platform setup, testing, and release guidance.
                    </p>
                  </section>
                </>
              )}
            </div>
            {page === 'workspace' && mode === 'host' && (
              <aside className="context-panel">
                <div className="context-visual">
                  <div className="device-line">
                    <div className="mini-device">
                      <Desktop size={25} />
                    </div>
                    <div className="connection-line">
                      <span />
                      <span />
                      <span />
                    </div>
                    <div className="mini-device accent">
                      <TerminalWindow size={25} />
                    </div>
                  </div>
                  <span>
                    YOUR DEVICE <span>THEIR TERMINAL</span>
                  </span>
                </div>
                <div className="context-copy">
                  <div className="eyebrow">A DIRECT WORKSPACE</div>
                  <h2>
                    Less back-and-forth.
                    <br />
                    More fixing.
                  </h2>
                  <p>
                    A native shell for the person or agent helping you. No port forwarding or
                    permanent remote access.
                  </p>
                </div>
                <ul className="assurance-list">
                  <li>
                    <CheckCircle size={18} />
                    Local approval before access
                  </li>
                  <li>
                    <CheckCircle size={18} />
                    Sessions expire automatically
                  </li>
                  <li>
                    <CheckCircle size={18} />
                    Stop sharing in one click
                  </li>
                  <li>
                    <CheckCircle size={18} />
                    Access events logged locally
                  </li>
                </ul>
                <div className="context-footer">
                  <ShieldCheck size={21} />
                  <p>
                    No access at launch.
                    <br />
                    You choose when to share.
                  </p>
                </div>
              </aside>
            )}
          </div>
        </main>
        <footer className="app-footer">
          <span>
            <LinkBreak size={14} />
            Temporary by design.
          </span>
          <span>
            © 2026{' '}
            <a href="https://spacie.net/" target="_blank" rel="noreferrer">
              Spacie
            </a>
            . All rights reserved.
          </span>
        </footer>
      </div>
    </div>
  );
}
