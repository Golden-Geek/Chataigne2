import { mount } from 'svelte';
import CurveFixture from './CurveFixture.svelte';
import GradientFixture from './GradientFixture.svelte';
mount(new URLSearchParams(location.search).has('gradient') ? GradientFixture : CurveFixture, {
	target: document.getElementById('app')!
});
