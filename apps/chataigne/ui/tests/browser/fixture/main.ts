import { mount } from 'svelte';
import AppTrees from './AppTrees.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Missing app tree fixture mount.');
const fixture = mount(AppTrees, { target });
declare global {
	interface Window {
		appTrees: typeof fixture;
	}
}
window.appTrees = fixture;
