import {
  ArrowClockwise,
  ArrowSquareOut,
  CheckCircle,
  DownloadSimple,
  ShieldCheck,
} from '@phosphor-icons/react';
import type { useUpdates } from '../hooks/useUpdates';
import { native } from '../services/desktop';
import { version } from '../../package.json';

export function SettingsPanel({
  updates,
  sessionBusy,
}: {
  updates: ReturnType<typeof useUpdates>;
  sessionBusy: boolean;
}) {
  const { state, release, progress, error, checkedAt } = updates;
  const working = ['checking', 'downloading', 'installing'].includes(state);
  const percent = progress.total
    ? Math.min(100, Math.round((progress.downloaded / progress.total) * 100))
    : undefined;
  const label =
    state === 'checking'
      ? 'Checking for updates…'
      : state === 'current'
        ? 'You’re up to date'
        : state === 'available'
          ? `Tether ${release?.version} is available`
          : state === 'downloading'
            ? 'Downloading the update'
            : state === 'installing'
              ? 'Installing the update…'
              : state === 'installed'
                ? 'Update installed. Restart to finish.'
                : state === 'error'
                  ? 'Update could not be completed'
                  : 'Keep Tether up to date';
  return (
    <>
      <section className="section-heading">
        <h1>Settings</h1>
        <p>Manage your installation and check for new releases.</p>
      </section>
      <section className="settings-panel">
        <div className="panel-heading">
          <h2>Software updates</h2>
          <span className="tag">Stable channel</span>
        </div>
        <div className="update-summary">
          <div className="update-symbol">
            <DownloadSimple size={26} />
          </div>
          <div>
            <strong role="status">{label}</strong>
            <p>
              Installed version <code>{version}</code>
              {checkedAt && ` · Last checked ${checkedAt.toLocaleTimeString()}`}
            </p>
          </div>
        </div>
        {error && (
          <p className="update-error" role="alert">
            {error} Check your connection and try again.
          </p>
        )}
        {state === 'downloading' && (
          <div className="download-progress">
            <progress aria-label="Update download progress" max="100" value={percent} />
            <span>
              {percent === undefined
                ? `${(progress.downloaded / 1048576).toFixed(1)} MB downloaded`
                : `${percent}%`}
            </span>
          </div>
        )}
        {release && (
          <details className="release-notes" open>
            <summary>What’s new in {release.version}</summary>
            <pre>{release.notes}</pre>
          </details>
        )}
        <div className="button-row">
          {state !== 'installed' && (
            <button
              className={release ? 'quiet' : 'primary'}
              disabled={!native || working}
              onClick={() => void updates.checkNow()}
            >
              <ArrowClockwise size={17} />
              {state === 'checking' ? 'Checking…' : 'Check for updates'}
            </button>
          )}
          {release && !['installed', 'checking'].includes(state) && (
            <button
              className="primary"
              disabled={sessionBusy || working}
              onClick={() => void updates.installNow()}
            >
              <DownloadSimple size={17} />
              {working ? 'Updating…' : 'Install update'}
            </button>
          )}
          {state === 'installed' && (
            <button className="primary" onClick={() => void updates.restart()}>
              <ArrowClockwise size={17} />
              Restart Tether
            </button>
          )}
        </div>
        {sessionBusy && release && (
          <p className="field-note">
            Stop sharing and disconnect from devices before installing an update.
          </p>
        )}
        {!native && (
          <p className="field-note">Update checks are available in the installed desktop app.</p>
        )}
        <p className="field-note">
          Linux updates keep your installation format. Debian updates may ask for administrator
          authentication.
        </p>
        <div className="settings-note">
          <ShieldCheck size={19} />
          <p>
            Update packages are verified with Spacie’s signing key before installation. Downloads
            start only when you choose to install.
          </p>
        </div>
      </section>
      <section className="settings-panel">
        <div className="panel-heading">
          <h2>About Tether</h2>
          <CheckCircle size={19} />
        </div>
        <p>
          Temporary remote terminals for developers and AI agents. Built by Spacie for Windows,
          macOS, and Linux.
        </p>
        <div className="about-links">
          <a href="https://github.com/abdvlrqhman/Tether/releases" target="_blank" rel="noreferrer">
            Release history <ArrowSquareOut size={15} />
          </a>
          <a href="https://spacie.net/" target="_blank" rel="noreferrer">
            spacie.net <ArrowSquareOut size={15} />
          </a>
        </div>
        <p className="field-note">© 2026 Spacie. All rights reserved.</p>
      </section>
    </>
  );
}
