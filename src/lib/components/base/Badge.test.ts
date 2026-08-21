import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/svelte';
import { createRawSnippet } from 'svelte';
import Badge from './Badge.svelte';

const text = (content: string) =>
  createRawSnippet(() => ({ render: () => `<span>${content}</span>` }));

function renderBadge(props: Record<string, unknown> = {}) {
  return render(Badge, {
    ...props,
    children: text('active'),
  });
}

describe('Badge', () => {
  function badgeElement() {
    return screen.getByText('active').closest('.badge');
  }

  it('renders children text', () => {
    renderBadge();
    expect(badgeElement()).toBeInTheDocument();
  });

  it('defaults to default variant class', () => {
    renderBadge();
    expect(badgeElement()).toHaveClass('badge-default');
  });

  it('applies variant class', () => {
    renderBadge({ variant: 'success' });
    expect(badgeElement()).toHaveClass('badge-success');
  });
});
