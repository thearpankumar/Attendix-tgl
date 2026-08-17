import { useEffect, useRef } from 'react';

interface Props {
  onIntersect: () => void;
  /** true while there's nothing left to load, or a fetch is already in flight. */
  disabled?: boolean;
}

// A near-invisible marker placed after a list — once it scrolls within
// `rootMargin` of the viewport, `onIntersect` fires to fetch and append the
// next page. `onIntersect` is read via a ref so the observer only needs to
// be (re)created when `disabled` changes, not on every render.
const InfiniteScrollSentinel = ({ onIntersect, disabled }: Props) => {
  const sentinelRef = useRef<HTMLDivElement>(null);
  const onIntersectRef = useRef(onIntersect);
  onIntersectRef.current = onIntersect;

  useEffect(() => {
    if (disabled) return;
    const el = sentinelRef.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries[0]?.isIntersecting) onIntersectRef.current();
      },
      { rootMargin: '200px' }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [disabled]);

  return <div ref={sentinelRef} aria-hidden="true" style={{ height: 1 }} />;
};

export default InfiniteScrollSentinel;
