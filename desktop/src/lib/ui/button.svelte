<!-- Adapted from shadcn-svelte Button; see THIRD_PARTY.md. -->
<script lang="ts" module>
  import { tv, type VariantProps } from 'tailwind-variants';
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import { clsx } from 'clsx';
  import { twMerge } from 'tailwind-merge';
  const buttonVariants = tv({
    base: 'inline-flex items-center justify-center rounded-md px-4 py-2 text-sm font-medium transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-sky-400 disabled:pointer-events-none disabled:opacity-50',
    variants: { variant: { default: 'bg-sky-400 text-slate-950 hover:bg-sky-300', secondary: 'bg-slate-800 text-slate-100 hover:bg-slate-700' } },
    defaultVariants: { variant: 'default' },
  });
  type Props = HTMLButtonAttributes & VariantProps<typeof buttonVariants>;
</script>
<script lang="ts">
  let { class: className, variant = 'default', type = 'button', children, ...rest }: Props = $props();
</script>
<button {type} class={twMerge(clsx(buttonVariants({ variant }), className))} {...rest}>{@render children?.()}</button>
