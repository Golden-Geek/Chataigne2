import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, readdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { openNativeEditor } from './native-editor.mjs';

const output = path.resolve(process.argv[2] ?? 'target/mapgyver-project-settings');
await mkdir(output, { recursive: true });
const directory = await mkdtemp(path.join(output, 'run-'));
const appDataDirectory = path.join(directory, 'profile');
const alpha = path.join(directory, 'Alpha.mapgyver');
const beta = path.join(directory, 'Beta.mapgyver');
const alphaFolder = path.join(directory, 'Alpha assets');
const betaFolder = path.join(directory, 'Beta assets');
await Promise.all([mkdir(alphaFolder), mkdir(betaFolder)]);
const pageErrors = [];
const observed = [];
let native, token, stage, failure;

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
	const value = await response.json();
	assert(response.ok, JSON.stringify(value));
	return value;
}

async function open() {
	native = await openNativeEditor({ appDataDirectory });
	native.page.on('pageerror', (error) => pageErrors.push(String(error)));
	token = (await post('/api/ui/session', {})).token;
}

async function close() {
	const owner = native;
	native = undefined;
	token = undefined;
	await owner?.close();
}

async function wait(read, accept) {
	const deadline = Date.now() + 5000;
	for (;;) {
		const value = await read();
		if (accept(value)) return value;
		assert(Date.now() < deadline, `${stage}: ${JSON.stringify(value)}`);
		await delay(60);
	}
}

const header = () => native.page.getByRole('banner');
const menu = () => header().getByRole('button', { name: 'Open File menu', exact: true });
const settingsButton = () =>
	header().getByRole('button', { name: 'Project Settings', exact: true });
const preferencesButton = () => header().getByRole('button', { name: 'Preferences', exact: true });
const folder = () => native.page.getByRole('textbox', { name: 'Project Folder', exact: true });
const oscHelp = () =>
	native.page
		.locator('.parameter-inspector')
		.filter({ has: native.page.getByText('Show OSC Address in Help', { exact: true }) })
		.getByRole('checkbox');

async function parameter(declId) {
	const { nodes } = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	return nodes.find((node) => node.decl_id === declId)?.data?.param?.value;
}

async function setFolder(value) {
	await folder().fill(value);
	await folder().press('Enter');
	await wait(
		() => parameter('project_folder'),
		(entry) => entry?.File === value
	);
}

async function expectProject(value) {
	await settingsButton().click();
	await wait(
		() => folder().inputValue(),
		(path) => path === value
	);
	assert.deepEqual(await parameter('project_folder'), { File: value });
}

async function expectPreference() {
	await preferencesButton().click();
	await wait(() => oscHelp().isChecked(), Boolean);
	assert.deepEqual(await parameter('show_osc_address_in_help'), { Bool: true });
}

async function save(file, value) {
	assert((await post('/api/ui/project-save', { path: file })).ok);
	const document = JSON.parse(await readFile(file, 'utf8'));
	const settings = document.root.children.find((node) => node.meta?.decl_id === 'project_settings');
	assert(settings?.children.some((node) => node.meta?.decl_id === 'project_folder'));
	assert(JSON.stringify(settings).includes(JSON.stringify(value)));
	assert(!document.root.children.some((node) => node.meta?.decl_id === 'preferences'));
}

async function load(file) {
	assert.equal((await post('/api/ui/project-load', { path: file })).path, file);
	// The transport can acknowledge the load before the workbench installs its snapshot.
	await wait(
		() => header().locator('.project-title').textContent(),
		(title) => title?.trim().replace(/\*$/, '') === path.basename(file)
	);
}

try {
	stage = 'open Project Settings from the File menu in a fresh MapGyver profile';
	await open();
	await menu().click();
	await native.page.getByRole('menuitem', { name: 'Project Settings', exact: true }).click();
	assert.equal(await folder().inputValue(), '');
	await native.page.getByTitle('Browse for a folder', { exact: true }).waitFor();
	const [projectIcon, preferenceIcon] = await Promise.all(
		[settingsButton(), preferencesButton()].map((button) =>
			button.locator('img').getAttribute('src')
		)
	);
	assert(projectIcon && preferenceIcon && projectIcon !== preferenceIcon);
	await setFolder(alphaFolder);
	observed.push('File menu opened the typed Project Folder editor');

	stage = 'edit app Preferences using its own top-bar button and save the first project';
	await preferencesButton().click();
	assert.equal(await oscHelp().isChecked(), false);
	await oscHelp().check();
	await wait(
		() => parameter('show_osc_address_in_help'),
		(value) => value?.Bool === true
	);
	await save(alpha, alphaFolder);

	stage = 'create and save a second project with its own folder';
	assert((await post('/api/ui/project-new', {})).ok);
	await expectProject('');
	await expectPreference();
	await settingsButton().click();
	await setFolder(betaFolder);
	await save(beta, betaFolder);

	stage = 'alternate between the saved projects without changing the app preference';
	for (const [file, value] of [
		[alpha, alphaFolder],
		[beta, betaFolder],
		[alpha, alphaFolder]
	]) {
		await load(file);
		await expectProject(value);
		await expectPreference();
	}
	observed.push(
		'New restored the default, then A/B/A restored distinct project folders with the same preference'
	);

	stage = 'restart MapGyver and verify the app-data Preferences file';
	const firstPid = native.app.pid;
	await close();
	const profile = await readdir(appDataDirectory, { recursive: true });
	assert(profile.some((file) => path.basename(file) === 'preferences.json'));
	await open();
	assert.notEqual(native.app.pid, firstPid);
	await expectPreference();
	await load(beta);
	await expectProject(betaFolder);
	await expectPreference();
	assert.deepEqual(pageErrors, []);
	observed.push('A new app process loaded the same preference and the saved Beta folder');
} catch (error) {
	failure = String(error.stack ?? error);
} finally {
	const appLog = failure ? native?.log() : undefined;
	try {
		await close();
	} catch (error) {
		failure = [failure, String(error.stack ?? error)].filter(Boolean).join('\n');
	}
	const receipt = path.join(directory, 'result.json');
	await writeFile(
		receipt,
		JSON.stringify(
			{ result: failure ? 'fail' : 'pass', stage, observed, pageErrors, failure, appLog },
			null,
			2
		)
	);
	assert(!failure, `${receipt}\n${failure}`);
	console.log(receipt);
}
