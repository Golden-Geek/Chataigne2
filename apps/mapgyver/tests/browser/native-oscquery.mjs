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
let token,
	failure,
	defaultPort,
	info,
	rootContents,
	oscEdit,
	managerCommands,
	lanControl,
	liveFeedback,
	addressTools;

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
	return result;
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

async function checkLiveFeedback() {
	const tree = await (await query('/screens')).json();
	const opacity = findOpacity(tree);
	const address = opacity.FULL_PATH;
	const changed = opacity.VALUE[0] === 0.625 ? 0.375 : 0.625;
	const socket = new WebSocket(`ws://127.0.0.1:${selectedPort}/`);
	socket.binaryType = 'arraybuffer';
	const next = (type) => once(socket, type, { signal: AbortSignal.timeout(5000) });
	try {
		await next('open');
		const acknowledged = next('message');
		socket.send(JSON.stringify({ COMMAND: 'LISTEN', DATA: address }));
		const [ack] = await acknowledged;
		assert.equal(JSON.parse(ack.data).DATA.ok, true);
		const feedback = next('message');
		const started = performance.now();
		await sendFloat(address, changed);
		const [event] = await feedback;
		const elapsedMs = performance.now() - started;
		const packet = Buffer.from(event.data);
		const end = packet.indexOf(0);
		assert.equal(packet.subarray(0, end).toString(), address);
		const tag = Math.ceil((end + 1) / 4) * 4;
		assert.equal(packet.subarray(tag, tag + 2).toString(), ',f');
		assert.equal(packet.readFloatBE(tag + 4), changed);
		assert(elapsedMs < 140, `OSCQuery feedback was delayed by ${elapsedMs}ms`);
		const snapshot = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
		const screen = snapshot.nodes.find((node) => node.meta.label === 'Screen');
		const oldIdentity = { node: screen.node_id, project_generation: snapshot.project_generation };
		await edit({
			kind: 'patchMeta',
			node: screen.node_id,
			patch: { short_name: 'oscBeforeReplacement' }
		});
		const oldHelp = '/screens/oscBeforeReplacement';
		await native.page
			.locator(`.outliner-item-content[data-node-id="${screen.node_id}"]:visible`)
			.first()
			.hover();
		await native.page.waitForFunction(
			(expected) => document.querySelector('.gc-footer .osc-address')?.textContent === expected,
			oldHelp,
			{ timeout: 5000 }
		);
		const closed = next('close');
		assert((await post('/api/ui/project-new', {})).ok);
		await closed;
		assert((await query('/?HOST_INFO'))?.ok);
		assert.equal((await post('/api/ui/osc-address', oldIdentity)).address, null);
		await native.page.waitForFunction(
			(retired) => document.querySelector('.gc-footer .osc-address')?.textContent !== retired,
			oldHelp,
			{ timeout: 5000 }
		);
		return { address, sent: changed, received: changed, elapsedMs, projectRetirement: 'pass' };
	} finally {
		socket.close();
	}
}

async function checkAddressTools() {
	const { page } = native;
	const snapshot = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const screens = snapshot.nodes.find((node) => node.decl_id === 'mapgyver_screen_manager');
	assert(screens, 'The Screen manager must be present in the workbench snapshot');
	const screen = snapshot.nodes.find(
		(node) => screens.children.includes(node.node_id) && node.meta.label === 'Screen'
	);
	const interfaceNode = snapshot.nodes.find((node) => node.decl_id === 'interface');
	const preference = snapshot.nodes.find(
		(node) =>
			interfaceNode.children.includes(node.node_id) && node.decl_id === 'show_osc_address_in_help'
	);
	assert(preference, 'The typed Interface preference must be installed');
	assert.equal(preference.data.param.value.Bool, false);
	const identity = { node: screen.node_id, project_generation: snapshot.project_generation };
	const lookup = async () => (await post('/api/ui/osc-address', identity)).address;
	const initial = await lookup();
	assert.equal(initial, '/screens/screen');
	assert.equal((await (await query(`${initial}?FULL_PATH`)).json()).FULL_PATH, initial);
	const row = page
		.locator(`.outliner-item-content[data-node-id="${screen.node_id}"]:visible`)
		.first();
	await row.hover({ timeout: 5000 });
	const help = page.locator('.gc-footer .osc-address');
	assert.equal(await help.count(), 0);
	await page.getByRole('button', { name: 'Open File menu' }).click();
	await page.getByRole('menuitem', { name: /^Preferences/ }).click();
	const section = page.locator(`.node-inspector[data-node-id="${interfaceNode.node_id}"]`).first();
	await section.waitFor({ timeout: 5000 });
	const field = page
		.locator(`.parameter-inspector[data-node-id="${preference.node_id}"] input[type="checkbox"]`)
		.first();
	if (!(await field.isVisible())) await section.locator('.node-title').first().click();
	await field.check({ timeout: 5000 });
	await row.hover();
	const shows = (value) =>
		page.waitForFunction(
			(expected) => document.querySelector('.gc-footer .osc-address')?.textContent === expected,
			value,
			{ timeout: 5000 }
		);
	await shows(initial);
	const created = await edit({
		kind: 'createUserItem',
		parent: screens.node_id,
		node_type: screen.node_type,
		label: 'OSC address test peer',
		initial_params: []
	});
	const duplicate = await lookup();
	assert.match(duplicate, /^\/screens\/~[0-9a-f]{32}$/);
	assert.equal((await (await query(`${duplicate}?FULL_PATH`)).json()).FULL_PATH, duplicate);
	await shows(duplicate);
	await page.evaluate(() => {
		Object.defineProperty(navigator, 'clipboard', {
			configurable: true,
			value: {
				writeText: async (value) => {
					window.oscAddressCopy = value;
				}
			}
		});
	});
	try {
		await row.click({ button: 'right' });
		await page.getByRole('menuitem', { name: 'Copy OSC Address', exact: true }).click();
		await page.waitForFunction((expected) => window.oscAddressCopy === expected, duplicate, {
			timeout: 5000
		});
	} finally {
		await page.evaluate(() => {
			delete navigator.clipboard;
			delete window.oscAddressCopy;
		});
	}
	await edit({ kind: 'removeNode', node: created.created_node.node_id });
	await row.hover();
	await shows(initial);
	return { identity, initial, copied: duplicate, liveCollisionAndRestore: 'pass' };
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
		addressTools = await checkAddressTools();
		managerCommands = await checkScreenCommands();
		if (process.argv.includes('--lan')) lanControl = await checkLanControl(allowLan);
		liveFeedback = await checkLiveFeedback();
	} else {
		assert((await post('/api/ui/project-new', {})).ok);
	}
	const replaced = (await post('/api/ui/snapshot', { scope: 'wholeGraph' })).nodes;
	const savedCategory = replaced.find((node) => node.decl_id === 'oscquery');
	assert(savedCategory.meta.enabled, 'OSCQuery enablement survives project replacement');
	const savedPort = replaced.find(
		(node) => savedCategory.children.includes(node.node_id) && node.decl_id === 'port'
	);
	assert.equal(savedPort.data.param.value.Int, selectedPort);
	if (addressTools) {
		const show = replaced.find((node) => node.decl_id === 'show_osc_address_in_help');
		assert.equal(show.data.param.value.Bool, true);
		assert.equal((await post('/api/ui/osc-address', addressTools.identity)).address, null);
		addressTools.projectReplacement = 'pass';
	}
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
				lanControl,
				liveFeedback,
				addressTools
			},
			null,
			2
		)
	);
}
console.log(JSON.stringify({ result: 'pass', defaultPort, selectedPort, rootContents }));
