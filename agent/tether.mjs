#!/usr/bin/env node
// Thin CLI presentation adapter. Secrets come from files/stdin, never URL parameters.
import { promises as fs } from 'node:fs';
import { parseArgs } from 'node:util';
import { setTimeout as delay } from 'node:timers/promises';
import WebSocket from 'ws';
import { request, validateURL, saveSession, loadSession, defaultSession } from './client.mjs';
const { values, positionals } = parseArgs({
  allowPositionals: true,
  options: {
    invitation: { type: 'string' },
    name: { type: 'string', default: 'Spacie agent' },
    session: { type: 'string', default: defaultSession },
    command: { type: 'string' },
    cwd: { type: 'string' },
    timeout: { type: 'string', default: '60' },
    json: { type: 'boolean', default: false },
    help: { type: 'boolean', short: 'h' },
  },
});
const action = positionals[0];
const help = `Tether by Spacie · https://spacie.net/\nCopyright © 2026 Spacie. All rights reserved.\n\nUsage: node agent/tether.mjs <pair|exec|terminal|status|logout> [options]\n\n  pair --invitation <private JSON file or -> --name <operator name>\n  exec --command <shell command> [--cwd <path>] [--timeout 1..300] [--json]\n  terminal                     Open an interactive native terminal\n  status                       Show session metadata (no credentials)\n  logout                       Revoke session and remove local credentials\n\n  --session <file>              Override private session file location\n  --help                       Show this help\n\nThe host must approve pairing in Tether. Shell commands use the host user's\npermissions. Keep invitations/session files private. Sessions expire.\n`;
async function pair() {
  if (!values.invitation)
    throw new Error('Specify --invitation with a private JSON file, or - to read stdin.');
  let source;
  if (values.invitation === '-') {
    const chunks = [];
    let length = 0;
    for await (const chunk of process.stdin) {
      length += chunk.length;
      if (length > 32768) throw new Error('Invitation too large.');
      chunks.push(chunk);
    }
    source = Buffer.concat(chunks).toString();
  } else {
    const stat = await fs.stat(values.invitation);
    if (stat.size > 32768) throw new Error('Invitation too large.');
    source = await fs.readFile(values.invitation, 'utf8');
  }
  const invitation = JSON.parse(source),
    url = validateURL(invitation.url);
  if (typeof invitation.invite !== 'string' || invitation.invite.length !== 43)
    throw new Error('Invalid invitation.');
  const receipt = await request(url, '/v1/pair', {
    method: 'POST',
    body: { invite: invitation.invite, operator: values.name },
  });
  process.stderr.write(
    `Waiting for the host to approve ${values.name}. Request ${receipt.id.slice(0, 8)}.\n`,
  );
  const deadline = Date.now() + 285000;
  while (Date.now() < deadline) {
    const result = await request(url, `/v1/pair/${receipt.id}`, { token: receipt.proof });
    if (result.status === 'approved') {
      await saveSession(values.session, {
        url,
        token: result.token,
        expires_at: result.expires_at,
        os: result.os,
        operator: values.name,
      });
      process.stdout.write(
        JSON.stringify({ status: 'connected', url, expires_at: result.expires_at, os: result.os }) +
          '\n',
      );
      return;
    }
    if (result.status !== 'pending')
      throw new Error(`Pairing ${result.status}. Ask for a new invitation.`);
    await delay(1000);
  }
  throw new Error('Host approval timed out. Ask for a new invitation.');
}
async function execute() {
  if (!values.command) throw new Error('Specify --command.');
  const timeout = Number(values.timeout);
  if (!Number.isInteger(timeout) || timeout < 1 || timeout > 300)
    throw new Error('Timeout must be between 1 and 300 seconds.');
  const session = await loadSession(values.session);
  const job = await request(session.url, '/v1/exec', {
    token: session.token,
    method: 'POST',
    body: { command: values.command, cwd: values.cwd, timeout_seconds: timeout },
  });
  let interrupted = false;
  const cancel = () => {
    interrupted = true;
    void request(session.url, `/v1/jobs/${job.id}`, {
      token: session.token,
      method: 'DELETE',
    }).catch(() => {});
  };
  process.once('SIGINT', cancel);
  process.once('SIGTERM', cancel);
  try {
    const deadline = Date.now() + (timeout + 20) * 1000;
    while (Date.now() < deadline) {
      const state = await request(session.url, `/v1/jobs/${job.id}`, { token: session.token });
      if (state.status === 'finished') {
        const result = state.result;
        if (values.json) process.stdout.write(JSON.stringify({ ...result, job_id: job.id }) + '\n');
        else {
          process.stdout.write(result.stdout);
          process.stderr.write(result.stderr);
          if (result.error) process.stderr.write(result.error + '\n');
          if (result.truncated) process.stderr.write('Output truncated at the host limit.\n');
        }
        process.exitCode = interrupted
          ? 130
          : result.cancelled
            ? 130
            : result.timed_out
              ? 124
              : result.error
                ? 1
                : Math.max(0, Math.min(255, result.exit_code ?? 1));
        return;
      }
      await delay(250);
    }
    cancel();
    throw new Error('Host did not finish the job in time; cancellation requested.');
  } finally {
    process.removeListener('SIGINT', cancel);
    process.removeListener('SIGTERM', cancel);
  }
}
async function terminal() {
  if (!process.stdin.isTTY || !process.stdout.isTTY)
    throw new Error('Interactive terminal requires a TTY. Use exec --json for automation.');
  const session = await loadSession(values.session);
  const url = session.url.replace(/^http/, 'ws') + '/v1/terminal';
  await new Promise((resolve, reject) => {
    const socket = new WebSocket(url, {
      headers: { authorization: `Bearer ${session.token}` },
      maxPayload: 32768,
      handshakeTimeout: 15000,
      followRedirects: false,
    });
    let open = false;
    const send = (data) => {
      if (socket.readyState === WebSocket.OPEN) socket.send(JSON.stringify(data));
    };
    const input = (chunk) => send({ type: 'input', data: chunk.toString('base64') });
    const resize = () =>
      send({
        type: 'resize',
        cols: process.stdout.columns || 100,
        rows: process.stdout.rows || 28,
      });
    const stop = () => socket.close();
    const cleanup = () => {
      if (open) process.stdin.setRawMode(false);
      process.stdin.pause();
      process.stdin.removeListener('data', input);
      process.stdout.removeListener('resize', resize);
      process.removeListener('SIGTERM', stop);
    };
    socket.on('open', () => {
      open = true;
      process.stdin.setRawMode(true);
      process.stdin.resume();
      process.stdin.on('data', input);
      process.stdout.on('resize', resize);
      process.once('SIGTERM', stop);
      resize();
    });
    socket.on('message', (data) => {
      try {
        const frame = JSON.parse(data.toString());
        if (frame.type === 'output') process.stdout.write(Buffer.from(frame.data, 'base64'));
        if (frame.type === 'error') process.stderr.write(frame.message + '\n');
      } catch {
        socket.close();
      }
    });
    socket.on('error', (error) => {
      cleanup();
      reject(error);
    });
    socket.on('close', () => {
      cleanup();
      resolve();
    });
  });
}
try {
  if (values.help || !action) {
    process.stdout.write(help);
  } else if (action === 'pair') await pair();
  else if (action === 'exec') await execute();
  else if (action === 'terminal') await terminal();
  else if (action === 'status') {
    const s = await loadSession(values.session);
    const result = await request(s.url, '/v1/session', { token: s.token });
    process.stdout.write(JSON.stringify(result) + '\n');
  } else if (action === 'logout') {
    const s = await loadSession(values.session).catch(() => null);
    if (s) {
      await request(s.url, '/v1/session', { token: s.token, method: 'DELETE' });
    }
    await fs.rm(values.session, { force: true });
    process.stdout.write('{"status":"logged_out"}\n');
  } else throw new Error(`Unknown command: ${action}`);
} catch (error) {
  if (values.json) process.stdout.write(JSON.stringify({ error: error.message }) + '\n');
  else process.stderr.write(`Tether: ${error.message}\n`);
  process.exitCode = 2;
}
