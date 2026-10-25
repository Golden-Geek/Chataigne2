import { mount } from 'svelte';
import Fixture from './Fixture.svelte';
const target = document.getElementById('app');
if (!target) throw new Error('Missing fixture mount.');
const fixture = mount(Fixture, { target });
declare global {
	interface Window {
		collectionFixture: typeof fixture;
	}
}
window.collectionFixture = fixture;
