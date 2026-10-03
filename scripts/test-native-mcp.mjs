// Verify the packaged binary's actual stdio protocol, without launching the desktop UI.
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { promises as fs } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { once } from 'node:events';
import assert from 'node:assert/strict';
const binary = resolve(
  process.argv[2] || `src-tauri/target/release/tether${process.platform === 'win32' ? '.exe' : ''}`,
);
await fs.access(binary);
const temporary = await fs.mkdtemp(join(tmpdir(), 'tether-mcp-native-'));
const child = spawn(binary, ['--mcp'], {
  windowsHide: true,
  stdio: ['pipe', 'pipe', 'pipe'],
  env: { ...process.env, TETHER_SESSION_FILE: join(temporary, 'session.json') },
});
const pending = new Map();
let id = 0,
  stderr = '';
child.stderr.on('data', (bytes) => {
  stderr += bytes.toString();
});
const lines = createInterface({ input: child.stdout });
lines.on('line', (line) => {
  try {
    const message = JSON.parse(line);
    const item = pending.get(message.id);
    if (item) {
      pending.delete(message.id);
      clearTimeout(item.timer);
      message.error
        ? item.reject(new Error(JSON.stringify(message.error)))
        : item.resolve(message.result);
    }
  } catch (error) {
    for (const item of pending.values())
      item.reject(new Error(`Non-protocol stdout: ${error.message}`));
  }
});
child.on('error', (error) => {
  for (const item of pending.values()) item.reject(error);
});
function call(method, params) {
  return new Promise((resolve, reject) => {
    const requestId = ++id;
    const timer = setTimeout(() => {
      pending.delete(requestId);
      reject(new Error(`MCP ${method} timed out. ${stderr}`));
    }, 10000);
    pending.set(requestId, { resolve, reject, timer });
    child.stdin.write(
      JSON.stringify({ jsonrpc: '2.0', id: requestId, method, ...(params ? { params } : {}) }) +
        '\n',
    );
  });
}
try {
  const initialized = await call('initialize', {
    protocolVersion: '2025-11-25',
    capabilities: {},
    clientInfo: { name: 'tether-native-test', version: '1' },
  });
  assert.equal(initialized.serverInfo.name, 'spacie-tether');
  assert.ok(initialized.capabilities.tools);
  child.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n');
  const tools = await call('tools/list');
  assert.equal(tools.tools.length, 7);
  assert.equal(
    tools.tools.find((tool) => tool.name === 'tether_exec').annotations.destructiveHint,
    true,
  );
  const status = await call('tools/call', { name: 'tether_session_status', arguments: {} });
  assert.equal(status.isError, true);
  assert.ok(status.structuredContent.error);
  assert.equal(status.structuredContent.token, undefined);
  const exit = once(child, 'exit');
  child.stdin.end();
  await Promise.race([
    exit,
    new Promise((_, reject) => {
      const timer = setTimeout(() => reject(new Error('MCP did not exit after EOF')), 5000);
      timer.unref();
    }),
  ]);
  assert.equal(child.exitCode, 0);
  console.log(
    'Native MCP passed: initialize, discovery, tool invocation, private-session error, and clean shutdown.',
  );
} finally {
  for (const item of pending.values()) clearTimeout(item.timer);
  lines.close();
  if (child.exitCode === null) child.kill();
  await fs.rm(temporary, { recursive: true, force: true });
}
