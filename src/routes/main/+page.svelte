<script lang="ts">
	import ShaderBackground from "../../components/ShaderBackground.svelte";
	import About from "../../components/viewblock/About.svelte";
	import LaunchButton from "../../components/LaunchButton.svelte";
	import Mods from "../../components/viewblock/Mods.svelte";
	import Settings from "../../components/viewblock/Settings.svelte";
	import RequiresPopup from "../../components/RequiresPopup.svelte";
	import type { DependencyCheck } from "../../stores/modStore";
	let currentSection = "mods";
	// window.addEventListener("resize", () => {
	//     console.log(
	//         `Window size: ${window.innerWidth} x ${window.innerHeight}`,
	//     );
	// });

	$: if (currentSection !== "mods") {
		// Store will retain the value but component won't show
		// Will reappear when returning to mods section
	}

	// Add these for the RequiresPopup
	let showRequiresPopup = false;
	let modRequirements = {
		steamodded: false,
		talisman: false,
	};

	function handleDependencyCheck(requirements: DependencyCheck) {
		modRequirements = requirements;
		showRequiresPopup = true;
	}
</script>

<ShaderBackground />
<div class="main-page">
	<header>
		<div class="header-content">
			<h1>Balatro Mod Manager</h1>
			<LaunchButton />
		</div>
		<nav>
			<button
				class:active={currentSection === "mods"}
				on:click={() => (currentSection = "mods")}
			>
				Mods
			</button>
			<button
				class:active={currentSection === "settings"}
				on:click={() => (currentSection = "settings")}
			>
				Settings
			</button>
			<button
				class:active={currentSection === "about"}
				on:click={() => (currentSection = "about")}
			>
				About
			</button>
		</nav>
	</header>

	<div class="content">
		{#if currentSection === "mods"}
			<Mods {handleDependencyCheck} />
		{/if}

		{#if currentSection === "settings"}
			<Settings />
		{/if}

		{#if currentSection === "about"}
			<About />
		{/if}
	</div>
	<RequiresPopup
		bind:show={showRequiresPopup}
		requiresSteamodded={modRequirements.steamodded}
		requiresTalisman={modRequirements.talisman}
	/>

	<div class="version-text">v0.1.0</div>
</div>

<style>
	:global(html) {
		font-size: 16px; /* Default root font-size */
	}
	@media (min-width: 768px) {
		:global(html) {
			font-size: 18px;
		}
	}
	@media (min-width: 1024px) {
		:global(html) {
			font-size: 20px;
		}
	}
	.main-page {
		width: 100vw;
		height: 100vh;
		display: flex;
		flex-direction: column;
		padding: 2rem;
		box-sizing: border-box;
		background: transparent;
	}
	header {
		margin-bottom: -1rem;
	}
	h1 {
		color: #f4eee0;
		font-size: 3rem;
		margin-bottom: 2rem;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-2px -2px 0 #000,
			2px -2px 0 #000,
			-2px 2px 0 #000,
			2px 2px 0 #000;
	}
	nav {
		display: flex;
		gap: 1rem;
		margin-bottom: 2rem;
	}
	button {
		background: transparent;
		border: 2px solid #f4eee0;
		color: #f4eee0;
		padding: 0.7rem 1.4rem;
		border-radius: 8px;
		font-family: "M6X11", sans-serif;
		font-size: 1.2rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	button:hover,
	button.active {
		background: #f4eee0;
		color: #393646;
	}
	.content {
		flex: 1;
		background: rgba(193, 65, 57, 0.8);
		border-radius: 5px;
		backdrop-filter: blur(10px);
		padding: 2rem;
		margin-bottom: 2rem;
		outline: 2px solid #f4eee0;
		overflow-y: auto;
		max-height: calc(100vh - 12rem);
		min-height: 0;
	}
	.content::-webkit-scrollbar {
		width: 10px;
	}
	.content::-webkit-scrollbar-track {
		background: transparent;
		border-radius: 15px;
	}
	.content::-webkit-scrollbar-thumb {
		background: #f4eee0;
		border: 2px solid rgba(193, 65, 57, 0.8);
		border-radius: 15px;
	}
	.content::-webkit-scrollbar:horizontal {
		display: none;
	}
	.content::-webkit-scrollbar-corner {
		background-color: transparent;
	}
	.version-text {
		position: fixed;
		bottom: 1rem;
		right: 1rem;
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		text-shadow:
			-1px -1px 0 #000,
			1px -1px 0 #000,
			-1px 1px 0 #000,
			1px 1px 0 #000;
	}
	.header-content {
		position: relative;
		margin-bottom: 2rem;
	}
	@media (max-width: 768px) {
		h1 {
			font-size: 2.5rem;
		}
		button {
			font-size: 1rem;
			padding: 0.5rem 1rem;
		}
		.main-page {
			padding: 1rem;
		}
	}
</style>
