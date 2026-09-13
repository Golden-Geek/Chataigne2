import { spawn } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:net';
import { join, resolve } from 'node:path';

const RESULT_PREFIX = 'PRODUCT_TRANSPORT_RESULT=';
const PROTOCOL_VERSION = '0.4.0';
const STARTUP_TIMEOUT_MS = 60_000;
const REQUEST_TIMEOUT_MS = 120_000;
const WORKBENCH_PLANES = ['structure', 'value', 'trigger', 'observation', 'catalog', 'preview'];

function delay(milliseconds) {
  return new Promise((done) => setTimeout(done, milliseconds));
}

async function availablePort() {
  const server = createServer();
  await new Promise((done, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', done);
  });
  const address = server.address();
  const port = address.port;
  await new Promise((done, reject) => server.close((error) => error ? reject(error) : done()));
  return port;
}

async function jsonRequest(baseUrl, path, body, timeoutMs = REQUEST_TIMEOUT_MS) {
  const response = await fetch(`${baseUrl}${path}`, {
    method: body === undefined ? 'GET' : 'POST',
    headers: body === undefined ? {} : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(timeoutMs),
  });
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${path} returned ${response.status}: ${text.slice(0, 500)}`);
  }
  return JSON.parse(text);
}

async function waitForHealth(baseUrl, child, predicate, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let lastError = '';
  while (Date.now() < deadline) {
    if (child.exitCode !== null) {
      throw new Error(`headless host exited with code ${child.exitCode}`);
    }
    try {
      const health = await jsonRequest(baseUrl, '/api/ui/health', undefined, 2_000);
      if (predicate(health)) return health;
    } catch (error) {
      lastError = String(error);
    }
    await delay(100);
  }
  throw new Error(`health condition timed out: ${lastError}`);
}

class ProbeClient {
  constructor(socket, name) {
    this.socket = socket;
    this.name = name;
    this.messages = [];
    this.waiters = [];
    this.closed = false;
    socket.addEventListener('message', (event) => {
      let message;
      try {
        message = JSON.parse(event.data);
      } catch (error) {
        this.failAll(new Error(`${name} received invalid JSON: ${error}`));
        return;
      }
      if (message.kind === 'error') {
        this.failAll(new Error(`${name} protocol error: ${message.message}`));
        return;
      }
      const index = this.waiters.findIndex((waiter) => waiter.matches(message));
      if (index >= 0) {
        const [waiter] = this.waiters.splice(index, 1);
        clearTimeout(waiter.timer);
        waiter.resolve(message);
      } else {
        if (message.kind !== 'delta' || resyncReason(message) !== null) {
          this.messages.push(message);
        }
      }
    });
    socket.addEventListener('close', () => {
      this.closed = true;
      this.failAll(new Error(`${name} WebSocket closed while awaiting a response`));
    });
    socket.addEventListener('error', (error) => {
      this.failAll(new Error(`${name} WebSocket error: ${error.message ?? error.type}`));
    });
  }

  static async connect(url, name) {
    const socket = new WebSocket(url);
    await new Promise((done, reject) => {
      const timer = setTimeout(() => reject(new Error(`${name} WebSocket open timed out`)), 10_000);
      socket.addEventListener('open', () => { clearTimeout(timer); done(); }, { once: true });
      socket.addEventListener('error', (error) => {
        clearTimeout(timer);
        reject(new Error(`${name} WebSocket open failed: ${error.message ?? error.type}`));
      }, { once: true });
    });
    return new ProbeClient(socket, name);
  }

  failAll(error) {
    for (const waiter of this.waiters.splice(0)) {
      clearTimeout(waiter.timer);
      waiter.reject(error);
    }
  }

  send(message) {
    if (this.closed) throw new Error(`${this.name} WebSocket is closed`);
    this.socket.send(JSON.stringify(message));
  }

  waitFor(matches, timeoutMs = REQUEST_TIMEOUT_MS) {
    const index = this.messages.findIndex(matches);
    if (index >= 0) return Promise.resolve(this.messages.splice(index, 1)[0]);
    return new Promise((resolveMessage, reject) => {
      const waiter = { matches, resolve: resolveMessage, reject, timer: null };
      waiter.timer = setTimeout(() => {
        this.waiters = this.waiters.filter((entry) => entry !== waiter);
        const queued = this.messages.slice(0, 5).map((message) => ({
          kind: message.kind,
          planes: message.deltas?.map((delta) => delta.plane),
          topics: message.deltas?.flatMap(
            (delta) => delta.batch?.events?.map((event) => event.topic).filter(Boolean) ?? [],
          ),
          first_event: message.deltas?.[0]?.batch?.events?.[0],
        }));
        reject(new Error(
          `${this.name} timed out waiting for a WebSocket response; `
          + `queued_count=${this.messages.length} first=${JSON.stringify(queued).slice(0, 1500)}`,
        ));
      }, timeoutMs);
      this.waiters.push(waiter);
    });
  }

  async close() {
    if (this.closed) return;
    const closed = new Promise((done) => this.socket.addEventListener('close', done, { once: true }));
    this.socket.close();
    await Promise.race([closed, delay(2_000)]);
  }
}

async function openSession(url, name) {
  const client = await ProbeClient.connect(url, name);
  client.send({ kind: 'hello', protocol_version: PROTOCOL_VERSION, client_instance_id: name });
  const hello = await client.waitFor((message) => message.kind === 'hello');
  if (hello.protocol_version !== PROTOCOL_VERSION) {
    throw new Error(`${name} negotiated ${hello.protocol_version}, expected ${PROTOCOL_VERSION}`);
  }
  return { client, hello };
}

async function snapshot(client, requestId) {
  client.send({ kind: 'snapshot', request_id: requestId, scope: 'wholeGraph' });
  const message = await client.waitFor((entry) => entry.kind === 'snapshot' && entry.request_id === requestId);
  return message.snapshot;
}

function snapshotCounts(snapshotValue, minimumNodes, expectedRoots) {
  const nodes = snapshotValue?.nodes;
  if (!Array.isArray(nodes) || nodes.length < minimumNodes) {
    throw new Error(`snapshot has ${nodes?.length ?? 'no'} nodes, below ${minimumNodes}`);
  }
  const roots = nodes.filter((node) => typeof node.decl_id === 'string' && node.decl_id.startsWith('scale_constant_'));
  if (roots.length !== expectedRoots) {
    throw new Error(`snapshot has ${roots.length} authored Constant roots, expected ${expectedRoots}`);
  }
  const identities = nodes.map((node) => `${node.uuid}\0${node.node_type}\0${node.decl_id}`).sort();
  const digest = createHash('sha256');
  for (const identity of identities) digest.update(identity).update('\n');
  return { nodes: nodes.length, roots: roots.length, node_identity_sha256: digest.digest('hex') };
}

function resyncReason(message) {
  if (message.kind === 'resyncRequired') return message.reason;
  if (message.kind !== 'delta') return null;
  for (const delta of message.deltas ?? []) {
    for (const event of delta.batch?.events ?? []) {
      if (event.kind === 'custom' && event.topic === '__transport.resync_required') {
        return event.payload?.reason ?? 'project_replaced';
      }
    }
  }
  return null;
}

async function run(binary, fixture, minimumNodes, expectedRoots, outputDir) {
  if (!existsSync(binary) || !existsSync(fixture)) throw new Error('binary or fixture does not exist');
  mkdirSync(outputDir, { recursive: true });
  const appData = join(outputDir, 'appdata');
  mkdirSync(appData, { recursive: true });
  const port = await availablePort();
  const baseUrl = `http://127.0.0.1:${port}`;
  const wsUrl = `ws://127.0.0.1:${port}/api/ui/ws`;
  const child = spawn(binary, ['--headless', '--no-remote', '--no-frontend'], {
    cwd: resolve('.'),
    env: { ...process.env, APPDATA: appData, GC_UI_BIND: `127.0.0.1:${port}` },
    stdio: ['ignore', 'pipe', 'pipe'],
    windowsHide: true,
  });
  let serverLog = '';
  child.stdout.on('data', (chunk) => { serverLog += chunk.toString(); });
  child.stderr.on('data', (chunk) => { serverLog += chunk.toString(); });
  const clients = [];
  try {
    await waitForHealth(
      baseUrl, child, (health) => health.backend_ready && health.engine_read_model_ready,
      STARTUP_TIMEOUT_MS,
    );
    const sessions = await Promise.all([0, 1, 2].map(async (index) => {
      const session = await openSession(wsUrl, `transport-probe-${index}`);
      clients.push(session.client);
      return session;
    }));
    const sessionIds = new Set(sessions.map(({ hello }) => hello.session_id));
    if (sessionIds.size !== 1) throw new Error('clients negotiated different runtime sessions');

    const initial = await Promise.all(sessions.map(({ client }, index) => snapshot(client, `before-${index}`)));
    for (const [index, session] of sessions.entries()) {
      session.client.send({
        kind: 'subscribe',
        subscription_id: 'workbench',
        interest: { view_id: `transport-probe-${index}`, scope: 'wholeGraph', planes: WORKBENCH_PLANES },
        from: initial[index].at,
      });
    }
    await waitForHealth(baseUrl, child, (health) => health.active_subscribed_websocket_clients === 3, 10_000);

    const loadStarted = performance.now();
    await jsonRequest(baseUrl, '/api/ui/project-load', { path: fixture, recover: false });
    const loadMs = Math.round(performance.now() - loadStarted);
    const resync = await Promise.all(sessions.map(({ client }) => client.waitFor(
      (message) => message.subscription_id === 'workbench' && resyncReason(message) !== null,
      30_000,
    )));

    const snapshots = await Promise.all(sessions.map(({ client }, index) => snapshot(client, `after-${index}`)));
    const counts = snapshots.map((value) => snapshotCounts(value, minimumNodes, expectedRoots));
    if (new Set(counts.map((entry) => entry.node_identity_sha256)).size !== 1) {
      throw new Error('the three clients received different node identities');
    }

    await sessions[0].client.close();
    await waitForHealth(baseUrl, child, (health) => health.active_subscribed_websocket_clients === 2, 10_000);
    const reconnected = await openSession(wsUrl, 'transport-probe-0');
    clients.push(reconnected.client);
    if (reconnected.hello.session_id !== sessions[0].hello.session_id) {
      throw new Error('reconnect entered a different runtime session');
    }
    const reconnectedSnapshot = await snapshot(reconnected.client, 'reconnected');
    const reconnectCounts = snapshotCounts(reconnectedSnapshot, minimumNodes, expectedRoots);
    if (reconnectCounts.node_identity_sha256 !== counts[0].node_identity_sha256) {
      throw new Error('the reconnected client received different node identities');
    }
    reconnected.client.send({
      kind: 'subscribe',
      subscription_id: 'workbench',
      interest: { view_id: 'transport-probe-0', scope: 'wholeGraph', planes: WORKBENCH_PLANES },
      from: reconnectedSnapshot.at,
    });
    const finalHealth = await waitForHealth(
      baseUrl, child, (health) => health.active_subscribed_websocket_clients === 3, 10_000,
    );
    return {
      contract: 'chataigne-product-transport-probe-v1',
      status: 'PASS',
      minimum_live_nodes: minimumNodes,
      graph_roots: expectedRoots,
      load_ms: loadMs,
      client_snapshots: counts,
      resync_reasons: resync.map(resyncReason),
      reconnect_snapshot: reconnectCounts,
      subscribed_clients_after_reconnect: finalHealth.active_subscribed_websocket_clients,
      session_consistent: true,
    };
  } finally {
    await Promise.all(clients.map((client) => client.close().catch(() => {})));
    if (child.exitCode === null) {
      child.kill();
      await Promise.race([new Promise((done) => child.once('exit', done)), delay(5_000)]);
    }
    writeFileSync(join(outputDir, 'headless-server.log'), serverLog);
  }
}

const [binaryArg, fixtureArg, minimumArg, rootsArg, outputArg] = process.argv.slice(2);
if (!binaryArg || !fixtureArg || !minimumArg || !rootsArg || !outputArg) {
  process.stderr.write('usage: node transport_probe.mjs BINARY FIXTURE MINIMUM_NODES GRAPH_ROOTS OUTPUT_DIR\n');
  process.exitCode = 2;
} else {
  try {
    const result = await run(
      resolve(binaryArg), resolve(fixtureArg), Number(minimumArg), Number(rootsArg), resolve(outputArg),
    );
    process.stdout.write(`${RESULT_PREFIX}${JSON.stringify(result)}\n`);
  } catch (error) {
    process.stderr.write(`transport probe failed: ${error.stack ?? error}\n`);
    process.exitCode = 1;
  }
}
