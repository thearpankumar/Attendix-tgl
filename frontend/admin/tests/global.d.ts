// Ambient type for the IntersectionObserver test double registered in
// setupTests.js — lets infinite-scroll tests grab the latest instance and
// fire its callback manually to simulate the sentinel scrolling into view.
export {};

declare global {
  // eslint-disable-next-line no-var
  var __intersectionObserverInstances: {
    callback: (entries: { isIntersecting: boolean }[]) => void;
  }[];
}
