import { describe, expect, it, vi } from 'vitest';
import type { UiCreateUserItemInitialParam } from 'golden_ui';
import {
	SoundCardRouteMutationController,
	type SoundCardRouteMutationPort
} from '../sound-card-route-mutations';

const createPort = () => {
	const create = vi.fn(
		async (
			_parent: number,
			_nodeType: string,
			_initialParams: readonly UiCreateUserItemInitialParam[]
		) => true
	);
	const setGain = vi.fn(async () => true);
	const remove = vi.fn(async () => true);
	const begin = vi.fn(async () => undefined);
	const end = vi.fn(async () => undefined);
	const port: SoundCardRouteMutationPort = {
		create,
		setGain,
		remove,
		createEditSession: () => ({ begin, end })
	};
	return { port, create, setGain, remove, begin, end };
};

describe('Sound Card route mutations', () => {
	it('creates a route with source, destination, and gain in one backend transaction', async () => {
		const fixture = createPort();
		const controller = new SoundCardRouteMutationController(fixture.port);

		expect(
			await controller.create({
				parent: 17,
				nodeType: 'sound_card_monitor_route',
				sourceDeclId: 'virtual_input',
				source: { kind: 'reference', uuid: 'input-uuid' },
				destinationDeclId: 'virtual_output',
				destination: { kind: 'reference', uuid: 'output-uuid' },
				gainDb: -3.5
			})
		).toBe(true);

		expect(fixture.create).toHaveBeenCalledOnce();
		const [parent, nodeType, initialParams] = fixture.create.mock.calls[0];
		expect(parent).toBe(17);
		expect(nodeType).toBe('sound_card_monitor_route');
		expect(initialParams).toEqual([
			{ decl_id: 'virtual_input', value: { kind: 'reference', uuid: 'input-uuid' } },
			{ decl_id: 'virtual_output', value: { kind: 'reference', uuid: 'output-uuid' } },
			{ decl_id: 'gain_db', value: { kind: 'float', value: -3.5 } }
		]);
	});

	it('updates only the backend gain parameter and removes by route node ID', async () => {
		const fixture = createPort();
		const controller = new SoundCardRouteMutationController(fixture.port);

		await controller.setGain({ parameter: 92, gainDb: -12, behaviour: 'Coalesce' });
		await controller.remove(41);

		expect(fixture.setGain).toHaveBeenCalledWith({
			parameter: 92,
			gainDb: -12,
			behaviour: 'Coalesce'
		});
		expect(fixture.remove).toHaveBeenCalledWith(41);
	});

	it('exposes one begin/end edit session for grouped matrix painting', async () => {
		const fixture = createPort();
		const controller = new SoundCardRouteMutationController(fixture.port);
		const session = controller.createEditSession('Paint monitor routes');

		await session.begin();
		await session.end();

		expect(fixture.begin).toHaveBeenCalledOnce();
		expect(fixture.end).toHaveBeenCalledOnce();
	});
});
