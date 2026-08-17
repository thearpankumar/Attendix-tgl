import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import Batches from '../../src/pages/Batches';
import axios from 'axios';
import { MemoryRouter } from 'react-router';

const makeBatch = (i: number) => ({
  _id: `b${i}`,
  name: `Batch ${i}`,
  description: '',
  studentCount: 5,
  createdAt: new Date(2026, 0, i + 1).toISOString(),
  type: 'manual' as const,
});

describe('Batches', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  const renderComponent = () => render(
    <MemoryRouter>
      <Batches />
    </MemoryRouter>
  );

  it('renders the empty state when there are no batches', async () => {
    (axios.get as any).mockResolvedValue({ data: [] });
    renderComponent();
    await waitFor(() => expect(screen.getByText(/No Batches Found/i)).toBeInTheDocument());
  });

  it('loads the first page (limit=30, offset=0) on mount', async () => {
    const page1 = Array.from({ length: 30 }, (_, i) => makeBatch(i));
    (axios.get as any).mockImplementation((_url: string, config?: { params?: { offset?: number } }) => {
      const offset = config?.params?.offset ?? 0;
      return Promise.resolve({ data: offset === 0 ? page1 : [] });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());
    expect(screen.getByText('Batch 29')).toBeInTheDocument();
    expect(axios.get).toHaveBeenCalledWith(
      '/api/admin/batches',
      expect.objectContaining({ params: { limit: 30, offset: 0 } })
    );
  });

  it('loads and appends the next page when the scroll sentinel intersects', async () => {
    const page1 = Array.from({ length: 30 }, (_, i) => makeBatch(i));
    const page2 = Array.from({ length: 5 }, (_, i) => makeBatch(30 + i));
    (axios.get as any).mockImplementation((_url: string, config?: { params?: { offset?: number } }) => {
      const offset = config?.params?.offset ?? 0;
      if (offset === 0) return Promise.resolve({ data: page1 });
      if (offset === 30) return Promise.resolve({ data: page2 });
      return Promise.resolve({ data: [] });
    });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());
    expect(screen.queryByText('Batch 34')).not.toBeInTheDocument();

    // The sentinel mounts (and registers its IntersectionObserver) in an
    // effect that fires after the page-1 rows have already painted, so wait
    // for the instance itself rather than assuming it exists as soon as the
    // text does — otherwise this is flaky under CI's slower scheduling.
    await waitFor(() => expect(globalThis.__intersectionObserverInstances.length).toBeGreaterThan(0));
    const [observer] = globalThis.__intersectionObserverInstances.slice(-1);
    observer.callback([{ isIntersecting: true }]);

    await waitFor(() => expect(screen.getByText('Batch 34')).toBeInTheDocument());
    // Original page-1 rows must still be there — appended, not replaced.
    expect(screen.getByText('Batch 0')).toBeInTheDocument();
  });

  it('does not render the scroll sentinel once every batch has been loaded', async () => {
    const fewBatches = Array.from({ length: 3 }, (_, i) => makeBatch(i));
    (axios.get as any).mockResolvedValue({ data: fewBatches });

    renderComponent();
    await waitFor(() => expect(screen.getByText('Batch 0')).toBeInTheDocument());

    // A page shorter than the page size means there's nothing more to load.
    expect(globalThis.__intersectionObserverInstances.length).toBe(0);
  });
});
