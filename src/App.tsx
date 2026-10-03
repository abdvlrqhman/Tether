import { useState } from 'react';
import {
  TerminalWindow,
  Desktop,
  ArrowsLeftRight,
  ClockCounterClockwise,
  Robot,
  GearSix,
  XCircle,
  ShieldCheck,
} from '@phosphor-icons/react';
import { useDesktop } from './hooks/useDesktop';
import { useUpdates } from './hooks/useUpdates';
import type { Mode, Page } from './contracts';
import { HostPanel } from './components/HostPanel';
import { OperatorPanel } from './components/OperatorPanel';
import { AgentPanel } from './components/AgentPanel';
import { SettingsPanel } from './components/SettingsPanel';
import { version } from '../package.json';
import spacieLogo from '../assets/spacie-logo.png';
import './styles.css';

export default function App() {
  const { data, busy, error, setError, run } = useDesktop();
  const updates = useUpdates();
  const [mode, setMode] = useState<Mode>('host'),
    [page, setPage] = useState<Page>('workspace');
  const { host, remote } = data;
  const active = !!host.session || remote.status === 'connected';
  const issue = error || host.error || remote.error;
  const controlsBusy = busy || ['installing', 'installed'].includes(updates.state);
  const names = {
    workspace: 'Devices',
    activity: 'Activity',
    agents: 'Agent access',
    about: 'Settings',
  };
  return (
    <div className="app-shell">
      <aside className="sidebar">
        <a
          className="brand"
          href="#"
          aria-label="Tether home"
          onClick={(e) => {
            e.preventDefault();
            setPage('workspace');
          }}
        >
          <span className="brand-mark">
            <TerminalWindow size={23} weight="bold" />
          </span>
          <span>
            Tether
            <small>
              by <img src={spacieLogo} alt="Spacie" />
            </small>
          </span>
        </a>
        <nav aria-label="Main navigation">
          <button
            className={'nav-item ' + (page === 'workspace' ? 'active' : '')}
            aria-current={page === 'workspace' ? 'page' : undefined}
            onClick={() => setPage('workspace')}
          >
            <Desktop size={20} />
            Devices
          </button>
          <button
            className={'nav-item ' + (page === 'activity' ? 'active' : '')}
            aria-current={page === 'activity' ? 'page' : undefined}
            onClick={() => setPage('activity')}
          >
            <ClockCounterClockwise size={20} />
            Activity
            {host.audit.length > 0 && <span className="nav-count">{host.audit.length}</span>}
          </button>
          <button
            className={'nav-item ' + (page === 'agents' ? 'active' : '')}
            aria-current={page === 'agents' ? 'page' : undefined}
            onClick={() => setPage('agents')}
          >
            <Robot size={20} />
            Agent access
          </button>
        </nav>
        <div className="sidebar-bottom">
          <div className="local-device">
            <span className={'status-dot ' + (host.running ? 'on' : '')} />
            <div>
              <strong>This device</strong>
              <small>{host.running ? 'Sharing is on' : 'Sharing is off'}</small>
            </div>
          </div>
          <button
            className={'nav-item ' + (page === 'about' ? 'active' : '')}
            aria-current={page === 'about' ? 'page' : undefined}
            onClick={() => setPage('about')}
          >
            <GearSix size={20} />
            Settings
            {updates.release && <span className="update-dot" aria-label="Update available" />}
          </button>
          <div className="sidebar-version">
            Tether {version}
            <span>© Spacie 2026</span>
          </div>
        </div>
      </aside>
      <div className="main-shell">
        <header className="topbar">
          <div className="breadcrumb">
            Workspace<span>/</span>
            <strong>{names[page]}</strong>
          </div>
          <div className="topbar-actions">
            <span className={'topbar-status ' + (active ? 'live' : '')}>
              <span className={'status-dot ' + (active ? 'on' : '')} />
              {active ? 'Session active' : host.running ? 'Ready to pair' : 'No active session'}
            </span>
            <button
              className="icon-button"
              aria-label="Software updates"
              title="Software updates"
              onClick={() => setPage('about')}
            >
              <DownloadIcon />
            </button>
          </div>
        </header>
        <main>
          {updates.state === 'installed' && (
            <div className="update-ready-banner" role="status">
              <span>Update installed. Restart Tether to finish.</span>
              <button className="primary small" onClick={() => void updates.restart()}>
                Restart now
              </button>
            </div>
          )}
          {page === 'workspace' && (
            <div className="workspace-tools">
              <div className="mode-switch" role="group" aria-label="Device role">
                <button
                  className={mode === 'host' ? 'selected' : ''}
                  aria-pressed={mode === 'host'}
                  onClick={() => setMode('host')}
                >
                  <Desktop size={18} />
                  Share this device
                </button>
                <button
                  className={mode === 'operator' ? 'selected' : ''}
                  aria-pressed={mode === 'operator'}
                  onClick={() => setMode('operator')}
                >
                  <ArrowsLeftRight size={18} />
                  Connect to a device
                </button>
              </div>
              <span className="workspace-hint">
                <ShieldCheck size={16} />
                Host approval required
              </span>
            </div>
          )}
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
          <div hidden={page !== 'workspace' || mode !== 'host'}>
            <HostPanel
              host={host}
              home={data.home}
              busy={controlsBusy}
              run={run}
              onError={setError}
            />
          </div>
          <div hidden={page !== 'workspace' || mode !== 'operator'}>
            <OperatorPanel remote={remote} busy={controlsBusy} run={run} onError={setError} />
          </div>
          {page === 'activity' && (
            <>
              <section className="section-heading">
                <h1>Activity</h1>
                <p>Access requests, approvals, and process events on this device.</p>
              </section>
              <section className="activity-panel">
                <div className="panel-heading">
                  <h2>Access history</h2>
                  <span className="tag">{host.audit.length} events</span>
                </div>
                {host.audit.length === 0 ? (
                  <div className="empty-state">
                    <ClockCounterClockwise size={34} />
                    <h2>No activity yet</h2>
                    <p>Start sharing to record access requests and sessions.</p>
                    <button
                      className="quiet"
                      onClick={() => {
                        setPage('workspace');
                        setMode('host');
                      }}
                    >
                      Share this device
                    </button>
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
                        <time dateTime={new Date(event.at * 1000).toISOString()}>
                          {new Date(event.at * 1000).toLocaleTimeString()}
                        </time>
                      </li>
                    ))}
                  </ol>
                )}
              </section>
              <p className="field-note">
                Local JSONL log: <code>{data.audit_path}</code>. The latest 200 events appear here.
                Commands and terminal output are not recorded.
              </p>
            </>
          )}
          {page === 'agents' && <AgentPanel executable={data.executable} onError={setError} />}
          {page === 'about' && (
            <SettingsPanel
              updates={updates}
              sessionBusy={busy || host.running || ['pending', 'connected'].includes(remote.status)}
            />
          )}
        </main>
      </div>
    </div>
  );
}
function DownloadIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.7"
      aria-hidden="true"
    >
      <path d="M12 3v12m-4-4 4 4 4-4M5 17v4h14v-4" />
    </svg>
  );
}
