<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon from './Icon.svelte';
  import type { IconName } from '../lib/icons';
  let {
    value = $bindable(''),
    placeholder = '',
    label,
    icon,
    password = false,
    mono = false,
    autofocus = false,
    invalid = false,
    describedby,
    onenter,
    onescape,
    oninput,
    onkeydown,
    input = $bindable<HTMLInputElement | undefined>(undefined),
    trailing,
  }: {
    value?: string;
    placeholder?: string;
    label: string;
    icon?: IconName;
    password?: boolean;
    mono?: boolean;
    autofocus?: boolean;
    invalid?: boolean;
    describedby?: string;
    onenter?: () => void;
    onescape?: () => void;
    oninput?: (v: string) => void;
    onkeydown?: (e: KeyboardEvent) => void;
    input?: HTMLInputElement;
    trailing?: Snippet;
  } = $props();
</script>

<label class="field" class:has-icon={!!icon} class:has-trailing={!!trailing} class:invalid>
  {#if icon}<span class="lead"><Icon name={icon} size={16} /></span>{/if}
  <!-- svelte-ignore a11y_autofocus -->
  <input
    bind:this={input}
    bind:value
    type={password ? 'password' : 'text'}
    class:id={mono && value.length > 0}
    {placeholder}
    {autofocus}
    aria-label={label}
    aria-invalid={invalid}
    aria-describedby={describedby}
    spellcheck="false"
    autocomplete="off"
    autocapitalize="off"
    oninput={() => oninput?.(value)}
    onkeydown={(e) => {
      onkeydown?.(e);
      if (e.defaultPrevented) return;
      if (e.key === 'Enter') onenter?.();
      if (e.key === 'Escape') onescape?.();
    }}
  />
  {#if trailing}<span class="trail">{@render trailing()}</span>{/if}
</label>

<style>
  .field {
    position: relative;
    display: flex;
    align-items: center;
    width: 100%;
    min-width: 0;
  }
  .lead {
    position: absolute;
    left: var(--sp-3);
    display: flex;
    color: var(--text-3);
    pointer-events: none;
  }
  .trail {
    position: absolute;
    right: var(--sp-1);
    display: flex;
    align-items: center;
    gap: var(--sp-1);
  }
  input {
    width: 100%;
    min-width: 0;
    height: var(--h-lg);
    padding: 0 var(--sp-3);
    border: 0;
    border-radius: var(--r-sm);
    background: var(--surface);
    box-shadow: inset 0 0 0 1px var(--line-strong);
    color: var(--text);
    font-size: var(--fs-md);
    outline: none;
    transition: box-shadow var(--t-fade) var(--ease-color);
  }
  input.id {
    font-size: var(--fs-md);
  }
  .has-icon input {
    padding-left: calc(var(--sp-3) + 16px + var(--sp-2));
  }
  .has-trailing input {
    padding-right: calc(var(--h-md) * 2);
  }
  input::placeholder {
    color: var(--text-3);
  }
  input:focus {
    box-shadow:
      inset 0 0 0 1px var(--accent),
      0 0 0 3px var(--accent-wash);
    transition: none;
  }
  .invalid input,
  .invalid input:focus {
    box-shadow:
      inset 0 0 0 1px var(--danger),
      0 0 0 3px var(--danger-wash);
  }
</style>
