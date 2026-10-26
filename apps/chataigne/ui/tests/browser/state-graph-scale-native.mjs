import assert from 'node:assert/strict';
import { runNativeGraphScale } from './app-graph-scale/native.mjs';

await runNativeGraphScale({
	consumer: 'state',
	fixture: new URL('./state-graph-fixture/', import.meta.url),
	globalName: 'stateGraphFixture',
	validateLoaded({ states }) {
		assert.equal(
			states.filter((state) => state.enabled).length,
			1,
			'Connected State fixture must have one backend-authoritative active State'
		);
	}
});
