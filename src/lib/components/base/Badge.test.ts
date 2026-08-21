import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import Badge from './Badge.svelte';

const text = (content: string) => createRawSnippet(() => ({ render: () => content }));

function renderBadge(props: Record<string, unknown> = {}) {
  return render(Badge, {
    ...props,
    children: text('active'),
  });
}

describe('Badge', () => {
  it('renders children text', () => {
    renderBadge();
    expect(screen.getByText('active')).toBeInTheDocument();
  });

  it('defaults to default variant class', () => {
    renderBadge();
    expect(screen.getByText('active')).toHaveClass('badge-default');
  });

  it('applies variant class', () => {
    renderBadge({ variant: 'success' });
    expect(screen.getByText('active')).toHaveClass('badge-success');
  });
});
