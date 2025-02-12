<script lang="ts">
	import { fade } from "svelte/transition";
	import { invoke } from "@tauri-apps/api/core";
	import { X } from "lucide-svelte";

	export let show: boolean = false;
	export let onClose: () => void;

	let isError: boolean = false;

	async function handleLaunch() {
		try {
			await invoke("launch_balatro");
			onClose();
			return;
		} catch (error) {
			console.error("Failed to launch game:", error);
			return;
		}
	}

	async function handleCheckAgain() {
		try {
			let steam_running: boolean = await invoke("check_steam_running");
			if (!steam_running) {
				isError = true;
				setTimeout(() => {
					isError = false;
				}, 2000);
			} else {
				await invoke("launch_balatro");
				onClose();
				return;
			}
		} catch (error) {
			console.error("Failed to check if Steam is running:", error);
		}
	}
</script>

{#if show}
	<div class="overlay" transition:fade={{ duration: 100 }}>
		<div class="alert-box">
			<button
				class="close-button"
				on:click={onClose}
				class:hidden={isError}
			>
				<div class="close-icon-container">
					<X size={13} />
				</div>
			</button>
			<div class="content" class:hidden={isError}>
				<h2 id="alert-title-first">
					Steam is not running. Are you sure?
				</h2>
				<div class="button-container">
					<button class="launch-button" on:click={handleLaunch}>
						Yes, launch without Steam
					</button>
					<button class="check-button" on:click={handleCheckAgain}>
						Check again
					</button>
				</div>
			</div>
			{#if isError}
				<div class="error-message" transition:fade={{ duration: 200 }}>
					<h2 id="alert-title" class:shake={isError}>
						Steam is still not running!
					</h2>
				</div>
			{/if}
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
	.overlay {
		position: fixed;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		background: rgba(0, 0, 0, 0.7);
		display: flex;
		justify-content: center;
		align-items: center;
		z-index: 1000;
	}
	.alert-box {
		background: #393646;
		border: 0.125rem solid #f4eee0; /* 2px */
		border-radius: 0.5rem; /* 8px */
		padding: 2rem;
		width: 25rem; /* 400px */
		text-align: center;
		position: relative;
		overflow: hidden;
	}
	.content {
		transition: opacity 0.2s;
		margin-bottom: 1rem;
	}
	.content.hidden {
		opacity: 0;
	}
	#alert-title {
		color: #f4eee0;
		margin-bottom: 2rem;
		font-family: "M6X11", sans-serif;
		font-size: 2.5rem;
	}
	#alert-title.shake {
		animation: shake 0.5s cubic-bezier(0.36, 0.07, 0.19, 0.97) both;
	}
	#alert-title-first {
		color: #f4eee0;
		font-family: "M6X11", sans-serif;
		font-size: 1.6rem;
	}
	.error-message {
		position: absolute;
		top: 0;
		left: 0;
		width: 100%;
		height: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		background: rgba(193, 65, 57, 0.9);
	}
	.close-button {
		position: absolute;
		top: 0.5rem;
		right: 0.5rem;
		background: transparent;
		border: none;
		cursor: pointer;
		padding: 0;
		z-index: 1000;
	}
	.close-button.hidden {
		display: none;
	}
	.close-icon-container {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 1.5625rem; /* 25px */
		height: 1.5625rem; /* 25px */
		border-radius: 0.25rem; /* 4px */
		background: #6e6e80;
		color: #f4eee0;
		transition: all 0.2s ease;
	}
	.close-button:hover .close-icon-container {
		background: #c14139;
		transform: scale(1.1);
		color: #f4eee0;
	}
	.close-button:active .close-icon-container {
		transform: scale(0.9);
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
	.launch-button {
		background: #c14139;
		border: 0.125rem solid #a13029; /* 2px */
		color: #f4eee0;
		padding: 0.75rem 1.5rem;
		border-radius: 0.25rem; /* 4px */
		cursor: pointer;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		transition: all 0.2s ease;
	}
	.launch-button:hover {
		background: #a13029;
		border: 0.125rem solid #c14139;
	}
	.launch-button:active {
		transform: translateY(0.125rem); /* 2px */
	}
	.check-button {
		background: #56a786;
		border: 0.125rem solid #459373; /* 2px */
		color: #f4eee0;
		padding: 0.75rem 1.5rem;
		border-radius: 0.25rem; /* 4px */
		cursor: pointer;
		font-family: "M6X11", sans-serif;
		font-size: 1.1rem;
		transition: all 0.2s ease;
	}
	.check-button:hover {
		background: #459373;
		border: 0.125rem solid #56a786;
	}
	.check-button:active {
		transform: translateY(0.125rem); /* 2px */
	}
</style>
