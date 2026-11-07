import { mount } from 'svelte';
import NumericFixture from './NumericFixture.svelte';
import '../../../style/golden-core.css';
const target = document.getElementById('app');
if (!target) throw Error('Missing numeric fixture mount');
const fixture = mount(NumericFixture, { target });
declare global {
	interface Window {
		numericFixture: typeof fixture;
	}
}
window.numericFixture = fixture;
