import { motion } from "framer-motion";
import { useEffect, useState } from "react";

/* ---------- Particles ---------- */
interface Particle {
  id: number;
  size: number;
  dur: number;
  left: number;
  top: number;
  delay: number;
}

export function Particles({ count = 40 }: { count?: number }) {
  // Math.random() must not run during render (it's impure and would produce
  // a server/client hydration mismatch under SSR) — generate particle data
  // client-side only, after mount.
  const [parts, setParts] = useState<Particle[]>([]);

  useEffect(() => {
    // No external system to synchronize with here — this is a one-time,
    // client-only randomization that must not run during (SSR) render.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setParts(
      Array.from({ length: count }, (_, i) => ({
        id: i,
        size: Math.random() * 2 + 1,
        dur: 10 + Math.random() * 20,
        left: Math.random() * 100,
        top: Math.random() * 100,
        delay: Math.random() * 5,
      })),
    );
  }, [count]);

  return (
    <div className="absolute inset-0 overflow-hidden pointer-events-none">
      {parts.map((p) => (
        <motion.span
          key={p.id}
          className="absolute rounded-full bg-white/60"
          style={{
            width: p.size,
            height: p.size,
            left: `${p.left}%`,
            top: `${p.top}%`,
            boxShadow: "0 0 8px rgba(255,255,255,0.6)",
          }}
          animate={{ y: [0, -60, 0], opacity: [0.1, 0.8, 0.1] }}
          transition={{
            duration: p.dur,
            repeat: Infinity,
            ease: "easeInOut",
            delay: p.delay,
          }}
        />
      ))}
    </div>
  );
}
