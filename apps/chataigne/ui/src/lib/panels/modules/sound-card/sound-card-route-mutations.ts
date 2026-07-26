import type {
	NodeId,
	ParamEventBehaviour,
	ParamValue,
	UiCreateUserItemInitialParam
} from 'golden_ui';
import {
	createUiEditSession,
	sendCreateUserItemByTypeIntent,
	sendRemoveNodeIntent,
	sendSetParamIntent
} from 'golden_ui/store/ui-intents';

export interface SoundCardRouteCreateRequest {
	readonly parent: NodeId;
	readonly nodeType: string;
	readonly sourceDeclId: string;
	readonly source: ParamValue;
	readonly destinationDeclId: string;
	readonly destination: ParamValue;
	readonly gainDb: number;
}

export interface SoundCardRouteGainRequest {
	readonly parameter: NodeId;
	readonly gainDb: number;
	readonly behaviour: ParamEventBehaviour;
}

export interface SoundCardRouteEditSession {
	begin(): Promise<void>;
	end(): Promise<void>;
}

export interface SoundCardRouteMutationPort {
	create(
		parent: NodeId,
		nodeType: string,
		initialParams: readonly UiCreateUserItemInitialParam[]
	): Promise<boolean>;
	setGain(request: SoundCardRouteGainRequest): Promise<boolean>;
	remove(node: NodeId): Promise<boolean>;
	createEditSession(label: string): SoundCardRouteEditSession;
}

const defaultPort: SoundCardRouteMutationPort = {
	async create(parent, nodeType, initialParams) {
		const result = await sendCreateUserItemByTypeIntent(parent, nodeType, undefined, {
			initial_params: [...initialParams],
			select_when_created: false
		});
		return result.success;
	},
	setGain: ({ parameter, gainDb, behaviour }) =>
		sendSetParamIntent(parameter, { kind: 'float', value: gainDb }, behaviour),
	remove: (node) => sendRemoveNodeIntent(node),
	createEditSession: (label) => createUiEditSession(label, 'sound-card-matrix')
};

export class SoundCardRouteMutationController {
	constructor(private readonly port: SoundCardRouteMutationPort = defaultPort) {}

	create(request: SoundCardRouteCreateRequest): Promise<boolean> {
		return this.port.create(request.parent, request.nodeType, [
			{
				decl_id: request.sourceDeclId,
				value: request.source
			},
			{
				decl_id: request.destinationDeclId,
				value: request.destination
			},
			{
				decl_id: 'gain_db',
				value: { kind: 'float', value: request.gainDb }
			}
		]);
	}

	setGain(request: SoundCardRouteGainRequest): Promise<boolean> {
		return this.port.setGain(request);
	}

	remove(node: NodeId): Promise<boolean> {
		return this.port.remove(node);
	}

	createEditSession(label: string): SoundCardRouteEditSession {
		return this.port.createEditSession(label);
	}
}
