import assert from 'node:assert/strict';
import { nativeWorkbench } from './native-workbench.mjs';

await nativeWorkbench({
	fixture: new URL('./spatializer-fixture/', import.meta.url),
	name: 'p11-spatializer-feedback',
	async exercise({ page, emit }) {
		await page.waitForFunction(
			() => window.spatializerFixture?.status().connection === 'connected'
		);
		const call = (method, ...args) =>
			page.evaluate(({ method, args }) => window.spatializerFixture[method](...args), {
				method,
				args
			});
		const fixture = await call('setup');
		const state = (id) =>
			page.evaluate((id) => {
				const f = window.spatializerFixture,
					owner = f.readNode(id);
				const field = (name) =>
					owner?.children
						.map((id) => f.readNode(id))
						.find((n) => n?.decl_id === name || n?.decl_id.endsWith('/' + name));
				const position = field('position_2d');
				const group = document.querySelector(`g.endpoint[data-endpoint-id="${id}"]`);
				const matrix = group?.transform.baseVal.consolidate()?.matrix;
				return {
					authored: position?.data.kind === 'parameter' ? position.data.param.value : null,
					x: matrix?.e,
					y: matrix?.f,
					freeze: Number(group?.querySelector('.freeze-radius-fill')?.getAttribute('r')),
					present: !!owner,
					pending: f.status().pending,
					feedback: document.querySelector('.edit-feedback')?.textContent
				};
			}, id);
		const held = () => page.waitForFunction(() => window.spatializerFixture.commandState().held);
		const settled = () =>
			page.waitForFunction(() => window.spatializerFixture.status().pending === 0);
		await call('reveal', fixture.source);
		await call('holdCommand');
		await call('startKeys', fixture.source, ['ArrowRight', 'ArrowRight']);
		await held();
		const before = await state(fixture.source);
		assert.deepEqual(before.authored, { kind: 'vec2', value: [0, 0] });
		assert.equal(before.x, 32);
		assert.ok(before.pending > 0);
		await call('releaseCommand');
		await call('finishCommand');
		await settled();
		const after = await state(fixture.source);
		assert.deepEqual(after.authored, { kind: 'vec2', value: [0.5, 0] });
		assert.equal(after.x, 32);
		emit({ kind: 'held-repeated-keyboard-feedback', passed: true, before, after });

		await call('reveal', fixture.west);
		await call('holdCommand');
		await call('startKeys', fixture.west, ['ArrowRight'], 'freezeRadius');
		await held();
		assert.equal((await state(fixture.west)).freeze, 16);
		await call('releaseCommand');
		await call('finishCommand');
		await settled();
		assert.equal((await state(fixture.west)).freeze, 16);
		emit({ kind: 'held-radius-feedback', passed: true });

		await call('reveal', fixture.source);
		await call('holdCommand');
		await call('startKeys', fixture.source, ['ArrowRight']);
		await held();
		assert.equal((await state(fixture.source)).x, 48);
		await call('remount');
		await call('reveal', fixture.source);
		assert.equal((await state(fixture.source)).x, 32);
		await call('releaseCommand');
		await call('finishCommand');
		await settled();
		await page.waitForFunction((id) => {
			const group = document.querySelector(`g.endpoint[data-endpoint-id="${id}"]`);
			return group?.transform.baseVal.consolidate()?.matrix.e === 48;
		}, fixture.source);
		assert.equal((await state(fixture.source)).feedback, '');
		emit({ kind: 'unmounted-command-retires-preview', passed: true });

		await call('reveal', fixture.east);
		await call('holdCommand');
		await call('startKeys', fixture.east, ['ArrowRight']);
		await held();
		assert.equal((await state(fixture.east)).x, 80);
		await call('remove', fixture.east);
		await call('releaseCommand');
		await call('finishCommand');
		await settled();
		assert.match((await call('commandState')).error, /Spatializer endpoint identity changed/);
		assert.equal((await state(fixture.east)).present, false);
		await call('history', 'undo');
		await call('reveal', fixture.east);
		assert.equal((await state(fixture.east)).x, 64);
		emit({ kind: 'rejected-command-withdraws-preview', passed: true });
	}
});
