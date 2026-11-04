// Real script editing and hosted callbacks; only the repository's finite process fixture runs.
import assert from 'node:assert/strict';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

export async function verifyApplicationScriptUi({
	page,
	editor,
	edit,
	post,
	intent,
	inspectMedia
}) {
	const { reference } = await edit({
		kind: 'CreateMedia',
		media_kind: 'Matrix',
		label: 'App script target'
	});
	const { snapshot } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'Inspect' }
	});
	const column = snapshot.columns[0].id;
	const layer = snapshot.items.find((item) => item.id.kind === 'Layer').id.id;
	const cellColumn = snapshot.columns[1].id;
	const { application: cell } = await edit({
		kind: 'Matrix',
		media: reference.media,
		edit: { kind: 'EnsureCell', layer, column: cellColumn }
	});
	const { command } = await edit({
		kind: 'CreateCommand',
		command_kind: 'UnloadMatrix',
		label: 'Script unload'
	});
	// This small fixture resolves the command's typed media Parameter through the public snapshot.
	const document = await post('/api/ui/snapshot', { scope: 'wholeGraph' });
	const commandNode = document.nodes.find((node) => node.uuid === command);
	assert(commandNode);
	const fields = await post('/api/ui/snapshot', {
		scope: { subtree: { root: commandNode.node_id, max_depth: 2 } }
	});
	const mediaParameter = fields.nodes.find((node) => node.decl_id === 'parameters/media');
	assert(mediaParameter);
	const configured = await intent({
		kind: 'setParam',
		node: mediaParameter.node_id,
		value: { Reference: { uuid: reference.media } },
		behaviour: 'Append'
	});
	assert(configured.success, JSON.stringify(configured));
	const runtime = async () =>
		(await edit({ kind: 'Applications', edit: { kind: 'InspectRuntime' } })).snapshot;
	const project = (await runtime()).project;
	const fixture = path.resolve('target/debug/application_process_fixture.exe');
	const source = `
const owner = ${JSON.stringify(column)}, cell = ${JSON.stringify(cell)};
const matrix = ${JSON.stringify(reference.media)}, layer = ${JSON.stringify(layer)};
const cellColumn = ${JSON.stringify(cellColumn)}, command = ${JSON.stringify(command)};
const fixture = ${JSON.stringify(fixture)}, project = ${JSON.stringify(project)};
let definition, ownedRuntime, pid, launch, sequence = 0, events = [], idle = false, unloaded = false, finished = false, runtimeReads = 0;
function require(value, message) { if (!value) throw new Error(message); }
function init() { local.listen({level: 1}); local.setApplicationFile('app-file', owner, fixture); }
function finish() {
  if (!idle || !unloaded || finished) return;
  finished = true;
  const phases = events.filter(e => e.status.runtime === ownedRuntime).map(e => e.status.phase);
  require(['Starting','Ready','Stopping','Idle'].every(phase => phases.includes(phase)), 'incomplete lifecycle: ' + phases);
  local.createMedia('idle-proof', 'SolidColor', 'App script idle ' + pid + ' ' + phases.join(','));
}
function onApplicationLifecycle(event) {
  require(event.project === project && event.sequence > sequence, 'stale or unordered lifecycle');
  sequence = event.sequence; events.push(event);
  if (event.status.phase === 'Ready' && !ownedRuntime) {
    ownedRuntime = event.status.runtime; pid = event.status.process_id; launch = event.status.launch;
    require(pid && launch, 'ready without owned identity');
    local.inspectApplicationRuntime('column-runtime');
  }
  if (event.status.runtime === ownedRuntime && event.status.phase === 'Idle') { idle = true; finish(); }
}
function onAppOperationResult(reply) {
  require(reply.status === 'applied', reply.request_id + ': ' + reply.message);
  switch(reply.request_id) {
    case 'app-file': local.inspectApplication('definition', owner); break;
    case 'definition':
      definition = reply.value.snapshot.definition;
      require(definition.executable === fixture, 'named file did not commit');
      definition.stop = { graceful_ms: 30, retirement_ms: 1000 }; definition.restart.enabled = false;
      local.configureApplication('configure-column', owner, JSON.stringify(definition)); break;
    case 'configure-column': local.configureApplication('configure-cell', cell, JSON.stringify(definition)); break;
    case 'configure-cell': local.launchMatrixColumn('launch-column', matrix, owner, 'Cut'); break;
    case 'column-runtime': {
      const current = reply.value.snapshot.applications.find(entry => entry.runtime === ownedRuntime);
      require(current && current.phase === 'Ready' && current.process_id === pid && current.consumers.length === 1, 'column runtime mismatch');
      local.launchMatrixCell('launch-cell', matrix, layer, cellColumn, 'Cut'); break;
    }
    case 'launch-cell': local.inspectApplicationRuntime('cell-runtime'); break;
    case 'cell-runtime': {
      const current = reply.value.snapshot.applications.find(entry => entry.runtime === ownedRuntime);
      if (!current || current.consumers.length !== 2) { require(++runtimeReads < 128, 'shared demand did not reconcile'); local.inspectApplicationRuntime('cell-runtime'); break; }
      require(current.process_id === pid && current.launch === launch, 'shared cell restarted owner');
      require(current.consumers.some(c => c.application === owner) && current.consumers.some(c => c.application === cell), 'wrong shared consumers');
      local.createMedia('ready-proof', 'SolidColor', 'App script ready ' + pid); break;
    }
    case 'ready-proof': local.inspectApplicationCallbacks('callbacks'); break;
    case 'callbacks':
      require(reply.value.state.fault === null, 'callback fault');
      local.executeCommand('command-unload', command); break;
    case 'command-unload': local.unloadMatrix('unload', matrix); break;
    case 'unload': unloaded = true; finish(); break;
    case 'idle-proof': local.setApplicationFile('clear-file', owner, ''); break;
  }
}
`;
	await editor.click();
	await editor.press('Control+a');
	await page.keyboard.insertText(source);
	const saving = page.waitForResponse((response) => response.url().endsWith('/script-config'));
	await editor.press('Control+s');
	assert((await saving).ok());
	await page.getByRole('button', { name: 'Reload Runtime', exact: true }).click();
	const deadline = Date.now() + 10_000;
	let ready, done;
	while (true) {
		const state = await inspectMedia();
		ready = state.media.find((media) => media.label.startsWith('App script ready '));
		done = state.media.find((media) => media.label.startsWith('App script idle '));
		const settings = (
			await edit({ kind: 'Applications', edit: { kind: 'Inspect', application: column } })
		).snapshot;
		if (ready && done && settings.definition.executable === null) break;
		assert(
			Date.now() < deadline,
			`hosted app script did not finish: ${JSON.stringify(await runtime())}`
		);
		await delay(30);
	}
	const pid = Number(ready.label.slice('App script ready '.length));
	assert(done.label.startsWith(`App script idle ${pid} `));
	assert.throws(
		() => process.kill(pid, 0),
		{ code: 'ESRCH' },
		'script/command unload must retire its owned process'
	);
	const final = await runtime();
	assert.equal(final.callbacks.fault, null);
	assert(final.applications.every((entry) => entry.process_id === null));
	console.log(
		JSON.stringify({
			result: 'pass',
			scope:
				'bundled script editor, eight named app methods, ordered actual lifecycle callbacks, Column/Cell same-PID sharing, script-executed typed unload command and named unload',
			pid,
			lifecycle: done.label,
			project,
			callbacks: final.callbacks
		})
	);
}
