import { writable } from 'svelte/store';

export type Theme = 'dark' | 'light';
const storageKey = 'workflow-factory.theme';
export const theme = writable<Theme>('dark');

function applyTheme(value: Theme) {
  document.documentElement.dataset.theme = value;
  document.documentElement.style.colorScheme = value;
  theme.set(value);
}

export function initializeTheme() {
  let value: Theme = 'dark';
  try {
    if (window.localStorage.getItem(storageKey) === 'light') value = 'light';
  } catch { /* Display remains available when local storage is blocked. */ }
  applyTheme(value);
}

export function setTheme(value: Theme) {
  applyTheme(value);
  try { window.localStorage.setItem(storageKey, value); }
  catch { /* Switching is still usable for this window. */ }
}
