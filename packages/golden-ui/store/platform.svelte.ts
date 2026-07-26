const isBrowser = typeof window !== 'undefined';

export const platform = $state({
	get name() {
		return isBrowser
			? (
					window as Window & {
						__PLATFORM__?: 'windows' | 'linux' | 'macos' | 'unknown';
					}
				).__PLATFORM__ || '?'
			: 'unknown';
	},
	get isLinux() {
		return this.name === 'linux';
	},
	get isWindows() {
		return this.name === 'windows';
	},
	get isMac() {
		return this.name === 'macos';
	}
});
