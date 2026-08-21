import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import Button from './Button.svelte';

// Svelte 5 renders snippet props only when they are real snippets
// (createSnippet / createRawSnippet), not plain arrow functions.
const text = (content: string) => createRawSnippet(() => ({ render: () => content }));

function renderButton(props: Record<string, unknown> = {}) {
  return render(Button, {
    ...props,
    children: text('Click me'),
  });
}

describe('Button', () => {
  it('renders children text', () => {
    renderButton();
    expect(screen.getByRole('button', { name: 'Click me' })).toBeInTheDocument();
  });

  it('applies variant and size classes', () => {
    renderButton({ variant: 'danger', size: 'lg' });
    const btn = screen.getByRole('button');
    expect(btn).toHaveClass('btn-danger');
    expect(btn).toHaveClass('btn-lg');
  });

  it('defaults to primary/md', () => {
    renderButton();
    const btn = screen.getByRole('button');
    expect(btn).toHaveClass('btn-primary');
    expect(btn).toHaveClass('btn-md');
  });

  it('fires onclick handler', async () => {
    const user = userEvent.setup();
    const handler = vi.fn();
    renderButton({ onclick: handler });
    await user.click(screen.getByRole('button'));
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('respects disabled without firing handler', async () => {
    const user = userEvent.setup();
    const handler = vi.fn();
    renderButton({ disabled: true, onclick: handler });
    const btn = screen.getByRole('button');
    expect(btn).toBeDisabled();
    await user.click(btn);
    expect(handler).not.toHaveBeenCalled();
  });
});
