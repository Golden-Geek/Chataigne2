// Verify bundled Golden OSCQuery Preferences and MapGyver's native OSC edit route.
import assert from 'node:assert/strict';
import { createSocket } from 'node:dgram';
import { once } from 'node:events';
import { mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { networkInterfaces } from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const directory = path.resolve(process.argv[2]);
const expected = process.argv.includes('--chataigne')
	? { title: 'Chataigne 2', port: 42000, roots: ['modulemanager', 'preferences'] }
	: { title: 'MapGyver', port: 43000, roots: ['medias', 'screens'] };
await mkdir(directory);
const reservation = createServer();
reservation.listen(0, '127.0.0.1');
await once(reservation, 'listening');
const selectedPort = reservation.address().port;
await new Promise((resolve) => reservation.close(resolve));
const native = await openNativeEditor();
let token, failure, defaultPort, info, rootContents, oscEdit, managerCommands, lanControl;

async function post(route, body) {
	const response = await fetch(native.base + route, {
		method: 'POST',
		headers: {
			'Content-Type': 'application/json',
			...(token ? { Authorization: `Bearer ${token}` } : {})
		},
		body: JSON.stringify(body),
		signal: AbortSignal.timeout(5000)
	});
	const result = await response.json();
	assert(response.ok, JSON.stringify(result));
	return result;
}

async function edit(payload) {
	const result = await post('/api/ui/intent', payload);
	assert(result.success, JSON.stringify(result));
}

async function query(suffix, host = '127.0.0.1') {
	try {
		return await fetch(`http://${host}:${selectedPort}${suffix}`, {
			signal: AbortSignal.timeout(250)
		});
	} catch {
		return null;
	}
}

async function wait(predicate) {
	const deadline = performance.now() + 6000;
	for (;;) {
		const result = await predicate();
		if (result) return result;
		assert(performance.now() < deadline, 'OSCQuery listener did not reach the requested state');
		await delay(35);
	}
}

function findOpacity(node) {
	if (node.FULL_PATH.endsWith('/opacity') && node.TYPE === 'f' && node.ACCESS === 3) return node;
	for (const child of Object.values(node.CONTENTS ?? {})) {
		const match = findOpacity(child);
		if (match) return match;
	}
}

async function sendFloat(address, value, host = '127.0.0.1') {
	const paddedAddress = Buffer.alloc(Math.ceil((Buffer.byteLength(address) + 1) / 4) * 4);
	paddedAddress.write(address);
	const args = Buffer.alloc(8);
	args.write(',f');
	args.writeFloatBE(value, 4);
	const client = createSocket('udp4');
	try {
		await new Promise((resolve, reject) => {
			client.send(Buffer.concat([paddedAddress, args]), selectedPort, host, (error) => {
				if (error) reject(error);
				else resolve();
			});
		});
	} finally {
		client.close();
	}
}

async function checkLanControl(allowLan) {
	const hosts = [
		...new Set(
			Object.values(networkInterfaces())
				.flat()
				.filter((entry) => entry.family === 'IPv4' && !entry.internal)
				.map((entry) => entry.address)
		)
	];
	assert(hosts.length, 'The LAN check requires a network adapter with an IPv4 address');
	const prior = new Map(
		await Promise.all(hosts.map(async (host) => [host, await query('/?HOST_INFO', host)]))
	);
	const set = (node, value) =>
		edit({ kind: 'setParam', node: node.node_id, value: { Bool: value }, behaviour: 'Append' });
	await set(allowLan, true);
	const host = await wait(async () => {
		for (const candidate of hosts) {
			const response = await query('/?HOST_INFO', candidate);
			if (response?.ok && (await response.json()).OSC_IP === candidate) return candidate;
		}
	});
	assert.equal(prior.get(host), null, 'MapGyver starts in local-only mode');
	const screens = await (await query('/screens', host)).json();
	const opacity = findOpacity(screens);
	const [original] = opacity.VALUE;
	const changed = original === 0.5 ? 0.25 : 0.5;
	await sendFloat(opacity.FULL_PATH, changed, host);
	await wait(async () => {
		const result = await query(`${opacity.FULL_PATH}?VALUE`, host);
		return result?.ok && (await result.json()).VALUE[0] === changed;
	});
	assert((await post('/api/ui/project-new', {})).ok);
	const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const category = nodes.find((node) => node.decl_id === 'oscquery');
	const saved = nodes.find(
		(node) => category.children.includes(node.node_id) && node.decl_id === 'allow_lan'
	);
	assert.equal(saved.data.param.value.Bool, true);
	assert((await query('/?HOST_INFO', host))?.ok);
	await set(saved, false);
	await wait(async () => !(await query('/?HOST_INFO', host)) && (await query('/?HOST_INFO'))?.ok);
	return {
		defaultLocalOnly: true,
		configuredPort: selectedPort,
		udpOpacity: changed,
		projectReplacement: 'pass',
		restoredLocalOnly: true
	};
}

async function checkScreenCommands() {
	const catalog = await (await query('/screens?GOLDEN_ADD_TYPES')).json();
	const screenType = catalog.GOLDEN_ADD_TYPES.find((item) => item.label === 'Screen').type;
	const socket = new WebSocket(`ws://127.0.0.1:${selectedPort}/`);
	const next = (type) => once(socket, type, { signal: AbortSignal.timeout(5000) });
	const command = async (name, data) => {
		const received = next('message');
		socket.send(JSON.stringify({ COMMAND: name, DATA: data, ID: name }));
		const [event] = await received;
		const reply = JSON.parse(event.data);
		assert.equal(reply.COMMAND, 'RESULT');
		assert.equal(reply.ID, name);
		assert(reply.DATA.ok, JSON.stringify(reply));
		return reply.DATA;
	};
	try {
		await next('open');
		const { address } = await command('ADD', {
			address: '/screens',
			type: screenType,
			name: 'OSCQuery Screen'
		});
		assert(address);
		const metadata = async () => (await (await query(`${address}?GOLDEN_META`)).json()).GOLDEN_META;
		assert.equal((await metadata()).label, 'OSCQuery Screen');
		await command('RENAME', { address, name: 'Live OSCQuery Screen' });
		assert.equal((await metadata()).label, 'Live OSCQuery Screen');
		const color = { r: 0.5, g: 0.25, b: 0.75, a: 1 };
		await command('PATCH_META', { address, meta: { color } });
		assert.deepEqual((await metadata()).color, color);
		await edit({ kind: 'undo' });
		assert.equal((await metadata()).color, null);
		await command('REDO');
		assert.deepEqual((await metadata()).color, color);
		await command('REMOVE', { address });
		assert.equal((await query(address)).status, 404);
		await command('UNDO');
		assert.equal((await metadata()).label, 'Live OSCQuery Screen');
		const video = await command('ADD', { address: '/medias', type: 'Video' });
		const videoMeta = await (await query(`${video.address}?GOLDEN_META`)).json();
		assert.match(videoMeta.GOLDEN_META.label, /^Video(?: \d+)?$/);
		await command('REMOVE', { address: video.address });
		return {
			address,
			addRenameColorRemoveUndoRedo: 'pass',
			videoCatalogLabel: videoMeta.GOLDEN_META.label
		};
	} finally {
		socket.close();
	}
}

try {
	token = (await post('/api/ui/session', {})).token;
	const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const category = nodes.find((node) => node.decl_id === 'oscquery');
	assert(category?.meta.can_be_disabled);
	assert.equal(category.meta.enabled, false);
	const port = nodes.find(
		(node) => category.children.includes(node.node_id) && node.decl_id === 'port'
	);
	const allowLan = nodes.find(
		(node) => category.children.includes(node.node_id) && node.decl_id === 'allow_lan'
	);
	assert.equal(allowLan.data.param.value.Bool, false);
	defaultPort = port.data.param.value.Int;
	assert.equal(defaultPort, expected.port);
	await edit({
		kind: 'setParam',
		node: port.node_id,
		value: { Int: selectedPort },
		behaviour: 'Append'
	});
	assert.equal(await query('/?HOST_INFO'), null);
	const enable = (node, enabled) =>
		edit({ kind: 'patchMeta', node: node.node_id, patch: { enabled } });
	await enable(category, true);
	info = await (
		await wait(async () => {
			const response = await query('/?HOST_INFO');
			return response?.ok && response;
		})
	).json();
	assert.equal(info.NAME, expected.title);
	assert.equal(info.OSC_IP, '127.0.0.1');
	assert.equal(info.OSC_PORT, selectedPort);
	assert.equal(info.OSC_TRANSPORT, 'UDP');
	assert.equal(info.WS_PORT, selectedPort);
	const project = await query('/');
	assert.equal(project.status, 200);
	const tree = await project.json();
	assert.equal(tree.FULL_PATH, '/');
	rootContents = Object.keys(tree.CONTENTS);
	for (const key of expected.roots) {
		assert(
			rootContents.some((actual) => actual.toLowerCase() === key),
			`Missing root ${key}`
		);
	}
	const portPath = await query('/preferences/oscquery/port?VALUE');
	assert.equal(portPath.status, 200);
	assert.deepEqual(await portPath.json(), { VALUE: [selectedPort] });
	if (expected.title === 'MapGyver') {
		const opacity = findOpacity(tree.CONTENTS.screens);
		assert(opacity, 'The default Surface publishes its writable Opacity address');
		const [original] = opacity.VALUE;
		const changed = original === 0.375 ? 0.625 : 0.375;
		const current = async () => (await (await query(`${opacity.FULL_PATH}?VALUE`)).json()).VALUE[0];
		await sendFloat(opacity.FULL_PATH, changed);
		await wait(async () => (await current()) === changed);
		await edit({ kind: 'undo' });
		assert.equal(await current(), original);
		await edit({ kind: 'redo' });
		assert.equal(await current(), changed);
		oscEdit = { address: opacity.FULL_PATH, original, changed, undoAndRedo: 'pass' };
		managerCommands = await checkScreenCommands();
		if (process.argv.includes('--lan')) lanControl = await checkLanControl(allowLan);
	}
	assert((await post('/api/ui/project-new', {})).ok);
	const replaced = (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
	const savedCategory = replaced.find((node) => node.decl_id === 'oscquery');
	assert(savedCategory.meta.enabled, 'OSCQuery enablement survives project replacement');
	const savedPort = replaced.find(
		(node) => savedCategory.children.includes(node.node_id) && node.decl_id === 'port'
	);
	assert.equal(savedPort.data.param.value.Int, selectedPort);
	assert((await query('/?HOST_INFO'))?.ok);
	await enable(savedCategory, false);
	await wait(async () => !(await query('/?HOST_INFO')));
} catch (error) {
	failure = String(error.stack ?? error);
	await writeFile(path.join(directory, 'app.log'), native.log());
	throw error;
} finally {
	await native.close();
	await writeFile(
		path.join(directory, 'result.json'),
		JSON.stringify(
			{
				result: failure ? 'fail' : 'pass',
				failure,
				defaultPort,
				selectedPort,
				info,
				rootContents,
				oscEdit,
				managerCommands,
				lanControl
			},
			null,
			2
		)
	);
}
console.log(JSON.stringify({ result: 'pass', defaultPort, selectedPort, rootContents }));
