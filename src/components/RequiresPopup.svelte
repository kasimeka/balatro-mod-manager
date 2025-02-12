<script lang="ts">
	import { CircleAlert } from "lucide-svelte";
	import { fade, scale } from "svelte/transition";
	import { invoke } from "@tauri-apps/api/core";

	export let show: boolean = false;
	export let requiresSteamodded: boolean = false;
	export let requiresTalisman: boolean = false;

	let steamoddedInstalled = false;
	let talismanInstalled = false;

	async function checkInstallations() {
		if (requiresSteamodded) {
			steamoddedInstalled = await invoke("check_mod_installation", {
				modType: "Steamodded",
			});
		}
		if (requiresTalisman) {
			talismanInstalled = await invoke("check_mod_installation", {
				modType: "Talisman",
			});
		}
	}

	$: if (show) {
		checkInstallations();
	}
</script>

{#if show}
	<div class="popup-overlay" transition:fade={{ duration: 100 }}>
		<div
			class="popup-content"
			transition:scale={{ duration: 200, start: 0.95, opacity: 1 }}
		>
			<div class="popup-header">
				<CircleAlert size={32} color="#fdcf51" />
				<h2>Required Dependencies</h2>
			</div>
			<div class="popup-body">
				<p>This mod requires the following dependencies:</p>
				<ul>
					{#if requiresSteamodded && !steamoddedInstalled}
						<li>
							<span class="dependency">Steamodded</span>
							- Core modding framework
						</li>
					{/if}
					{#if requiresTalisman && !talismanInstalled}
						<li>
							<span class="dependency">Talisman</span>
							- Extended modding API
						</li>
					{/if}
				</ul>

				<div class="button-container">
					<button
						class="cancel-button"
						on:click={() => (show = false)}
					>
						Close
					</button>
				</div>
			</div>
		</div>
	</div>
{/if}

<style>
	:global(html) {
		font-size: 16px; /* Base font size */
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
	.popup-overlay {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: rgba(0, 0, 0, 0.8);
		display: flex;
		justify-content: center;
		align-items: center;
		z-index: 1000;
	}
	.popup-content {
		background: #393646;
		border: 0.125rem solid #f4eee0; /* 2px */
		border-radius: 0.75rem; /* 12px */
		padding: 2rem;
		width: 31.25rem; /* 500px */
		max-width: 90%;
	}
	.popup-header {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		margin-bottom: 1.5rem;
	}
	.popup-header h2 {
		color: #fdcf51;
		font-size: 2rem;
		margin: 0;
	}
	.popup-body {
		color: #f4eee0;
		font-size: 1.2rem;
	}
	.popup-body p {
		margin-bottom: 1.5rem;
	}
	.popup-body ul {
		list-style: none;
		padding: 0;
		margin-bottom: 2rem;
	}
	.popup-body li {
		margin-bottom: 1rem;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		font-size: 1.2rem;
	}
	.dependency {
		color: #fdcf51;
		font-weight: bold;
		font-size: 1.3rem;
	}
	.button-container {
		display: flex;
		gap: 1rem;
		justify-content: flex-end;
	}
	.cancel-button {
		padding: 1rem 1.5rem;
		background: #c14139;
		color: #f4eee0;
		border: none;
		outline: 0.125rem solid #a13029; /* 2px */
		border-radius: 0.375rem; /* 6px */
		font-family: "M6X11", sans-serif;
		font-size: 1.2rem;
		cursor: pointer;
		transition: all 0.2s ease;
	}
	.cancel-button:hover {
		background: #d4524a;
		transform: translateY(-0.125rem); /* -2px */
	}
	@keyframes shake {
		10%,
		90% {
			transform: translate3d(-0.0625rem, 0, 0); /* -1px */
		}
		20%,
		80% {
			transform: translate3d(0.125rem, 0, 0); /* 2px */
		}
		30%,
		50%,
		70% {
			transform: translate3d(-0.25rem, 0, 0); /* -4px */
		}
		40%,
		60% {
			transform: translate3d(0.25rem, 0, 0); /* 4px */
		}
	}
	@media (max-width: 1160px) {
		.popup-content {
			padding: 1.5rem;
			width: 90%;
			max-width: 25rem; /* 400px */
		}
		.popup-header h2 {
			font-size: 1.5rem;
		}
		.popup-body {
			font-size: 1rem;
		}
		.popup-body li {
			font-size: 1rem;
			margin-bottom: 0.75rem;
		}
		.dependency {
			font-size: 1.1rem;
		}
		.cancel-button {
			padding: 0.75rem 1.25rem;
			font-size: 1rem;
			border-radius: 0.25rem; /* 4px */
		}
		.popup-header {
			gap: 0.5rem;
			margin-bottom: 1rem;
		}
	}
</style>
