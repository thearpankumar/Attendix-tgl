import { useRef, useState } from "react";

/* ---------- Magnetic Button ---------- */
export function MagneticButton({
  children,
  href,
  primary,
}: {
  children: React.ReactNode;
  href: string;
  primary?: boolean;
}) {
  const ref = useRef<HTMLAnchorElement>(null);
  const [pos, setPos] = useState({ x: 0, y: 0 });
  return (
    <a
      ref={ref}
      href={href}
      onMouseMove={(e) => {
        const r = ref.current!.getBoundingClientRect();
        setPos({
          x: (e.clientX - r.left - r.width / 2) * 0.25,
          y: (e.clientY - r.top - r.height / 2) * 0.25,
        });
      }}
      onMouseLeave={() => setPos({ x: 0, y: 0 })}
      className={`group relative inline-flex items-center gap-2 rounded-full px-6 py-3 text-sm font-medium transition-all duration-300 ${
        primary ? "bg-white text-black hover:bg-white/90" : "glass hover:bg-white/10 text-white"
      }`}
      style={{ transform: `translate(${pos.x}px, ${pos.y}px)` }}
    >
      {primary && (
        <span className="absolute inset-0 rounded-full bg-gradient-to-r from-cyan via-electric to-purple opacity-0 group-hover:opacity-20 blur-xl transition" />
      )}
      <span className="relative flex items-center gap-2">{children}</span>
    </a>
  );
}
