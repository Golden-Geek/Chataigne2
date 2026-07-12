export interface RuntimeLocation {
	readonly origin: string;
}

export interface RuntimeEndpointOptions {
	readonly development: boolean;
	readonly developmentBackendPort?: number;
}

export interface RuntimeEndpoints {
	readonly httpBaseUrl: string;
	readonly wsUrl: string;
}

const runtimeOrigin = (
	location: RuntimeLocation,
	{ development, developmentBackendPort = 7010 }: RuntimeEndpointOptions
): URL => {
	const origin = new URL(location.origin);
	if (development) {
		origin.port = String(developmentBackendPort);
	}
	return origin;
};

export const resolveRuntimeEndpoints = (
	location: RuntimeLocation,
	options: RuntimeEndpointOptions
): RuntimeEndpoints => {
	const httpOrigin = runtimeOrigin(location, options);
	const websocketOrigin = new URL(httpOrigin);
	websocketOrigin.protocol = httpOrigin.protocol === 'https:' ? 'wss:' : 'ws:';

	return {
		httpBaseUrl: new URL('/api/ui', httpOrigin).toString().replace(/\/$/, ''),
		wsUrl: new URL('/api/ui/ws', websocketOrigin).toString()
	};
};
