import assert from 'node:assert/strict';
import { runNativeGraphScale } from './app-graph-scale/native.mjs';

await runNativeGraphScale({
	consumer: 'formula',
	fixture: new URL('./alchemist-graph-fixture/', import.meta.url),
	globalName: 'alchemistGraphFixture',
	validateLoaded({ states, topology }) {
		assert.equal(states[0].type, 'constant');
		assert.ok(states.slice(1).every((state) => state.type === 'math'));
		for (let index = 0; index < topology.length; index++) {
			assert.equal(topology[index].sourceSocket, index === 0 ? 'value' : 'result');
			assert.equal(topology[index].targetSocket, 'value1');
		}
		return { anodeTypes: { constant: 1, math: states.length - 1 } };
	}
});
