import { useEffect } from "react";

/* ---------- Smooth scroll ---------- */
export function useLenis() {
  useEffect(() => {
    let lenis: InstanceType<Awaited<typeof import("lenis")>["default"]> | undefined;
    let raf: number;
    (async () => {
      const Lenis = (await import("lenis")).default;
      lenis = new Lenis({ duration: 1.2, smoothWheel: true, lerp: 0.08 });
      const loop = (t: number) => {
        lenis?.raf(t);
        raf = requestAnimationFrame(loop);
      };
      raf = requestAnimationFrame(loop);
    })();
    return () => {
      cancelAnimationFrame(raf);
      lenis?.destroy?.();
    };
  }, []);
}
