import { motion } from "framer-motion";
import {
  Cpu,
  Eye,
  Fingerprint,
  KeyRound,
  MapPin,
  ScanFace,
  ShieldCheck,
  Sparkles,
} from "lucide-react";
import { useEffect, useState } from "react";

import { Section } from "./primitives/Section";

/* ---------- Section 3: Engine (orbit) ---------- */
export function EngineSection() {
  const orbits = [
    { icon: ScanFace, label: "Face Recognition", color: "text-cyan" },
    { icon: KeyRound, label: "Passkeys", color: "text-electric" },
    { icon: Fingerprint, label: "Biometrics", color: "text-purple" },
    { icon: MapPin, label: "Geo-location", color: "text-emerald" },
    { icon: Cpu, label: "Device Validation", color: "text-cyan" },
    { icon: Sparkles, label: "AI Analysis", color: "text-electric" },
    { icon: Eye, label: "Liveness", color: "text-purple" },
    { icon: ShieldCheck, label: "Fraud Detection", color: "text-emerald" },
  ];
  const [activeIndex, setActiveIndex] = useState(0);

  useEffect(() => {
    const interval = setInterval(() => {
      setActiveIndex((prev) => (prev + 1) % orbits.length);
    }, 2500);
    return () => clearInterval(interval);
  }, [orbits.length]);

  return (
    <Section
      id="platform"
      eyebrow="The engine"
      title={
        <>
          One core. <span className="text-gradient-accent">Every signal.</span>
        </>
      }
      subtitle="The Attendix Intelligence Engine fuses eight verification layers into a single decision — in under 800 milliseconds."
    >
      {/* Desktop layout: Concentric orbits */}
      <div className="relative mx-auto aspect-square w-full max-w-[720px] hidden md:block">
        {/* outer rings */}
        {[1, 2, 3].map((r) => (
          <div
            key={r}
            className="absolute inset-0 rounded-full border border-white/[0.06]"
            style={{ margin: `${r * 40}px` }}
          />
        ))}
        {/* orbit paths */}
        <motion.div
          animate={{ rotate: 360 }}
          transition={{ duration: 60, repeat: Infinity, ease: "linear" }}
          className="absolute inset-16"
        >
          <div className="relative h-full w-full rounded-full border border-dashed border-white/[0.08]" />
        </motion.div>

        {/* orbiting icons */}
        {orbits.map((o, i) => {
          const angle = (i / orbits.length) * Math.PI * 2;
          const radius = 44; // percentage
          const x = 50 + Math.cos(angle) * radius;
          const y = 50 + Math.sin(angle) * radius;
          return (
            <motion.div
              key={o.label}
              initial={{ opacity: 0, scale: 0.6 }}
              whileInView={{ opacity: 1, scale: 1 }}
              viewport={{ once: true }}
              transition={{ delay: i * 0.08, duration: 0.5 }}
              className="absolute -translate-x-1/2 -translate-y-1/2"
              style={{ left: `${x}%`, top: `${y}%` }}
            >
              <div className="group relative">
                <motion.div
                  animate={{ y: [0, -6, 0] }}
                  transition={{ duration: 3 + i * 0.3, repeat: Infinity }}
                  className="glass-strong rounded-2xl p-3.5 grid place-items-center hover:scale-110 transition cursor-pointer"
                >
                  <o.icon className={`h-5 w-5 ${o.color}`} />
                </motion.div>
                <div className="mt-2 text-center text-[11px] text-white/70 whitespace-nowrap">
                  {o.label}
                </div>
              </div>
            </motion.div>
          );
        })}

        {/* core */}
        <div className="absolute inset-1/3 grid place-items-center">
          <motion.div
            animate={{ scale: [1, 1.05, 1] }}
            transition={{ duration: 3, repeat: Infinity }}
            className="relative aspect-square w-full rounded-full glass-strong grid place-items-center overflow-hidden"
          >
            <div className="absolute inset-0 bg-gradient-to-br from-cyan/30 via-electric/20 to-purple/30 blur-xl" />
            <div className="relative text-center">
              <div className="text-[10px] uppercase tracking-[0.3em] text-white/60">Attendix</div>
              <div className="mt-1 font-display text-2xl md:text-3xl text-gradient-accent">
                Core
              </div>
              <div className="mt-1 text-[10px] text-white/50">v4.2 · 812ms</div>
            </div>
          </motion.div>
        </div>

        {/* connecting lines pulse */}
        <svg className="absolute inset-0 h-full w-full pointer-events-none">
          {orbits.map((_, i) => {
            const angle = (i / orbits.length) * Math.PI * 2;
            const x = 50 + Math.cos(angle) * 44;
            const y = 50 + Math.sin(angle) * 44;
            return (
              <line
                key={i}
                x1="50%"
                y1="50%"
                x2={`${x}%`}
                y2={`${y}%`}
                stroke="rgba(255,255,255,0.05)"
              />
            );
          })}
        </svg>
      </div>

      {/* Mobile-optimized core & layers */}
      <div className="block md:hidden w-full max-w-[480px] mx-auto mt-4 px-2">
        <div className="flex flex-col items-center justify-center mb-6">
          <motion.div
            animate={{
              scale: [1, 1.03, 1],
              boxShadow: [
                "0 0 25px rgba(34,211,238,0.15)",
                "0 0 45px rgba(34,211,238,0.35)",
                "0 0 25px rgba(34,211,238,0.15)",
              ],
            }}
            transition={{ duration: 3, repeat: Infinity, ease: "easeInOut" }}
            className="relative h-28 w-28 rounded-full glass-strong grid place-items-center overflow-hidden border border-white/10"
          >
            <div className="absolute inset-0 bg-gradient-to-br from-cyan/20 via-electric/15 to-purple/20 blur-lg" />
            <div className="relative text-center z-10">
              <div className="text-[9px] uppercase tracking-[0.25em] text-white/50">Attendix</div>
              <div className="mt-0.5 font-display text-lg font-bold text-gradient-accent">Core</div>
              <div className="mt-1 text-[8px] text-white/40">v4.2 · 812ms</div>
            </div>
          </motion.div>
          <div className="h-6 w-px bg-gradient-to-b from-white/20 to-white/5" />
        </div>

        <div className="grid grid-cols-2 gap-3 w-full">
          {orbits.map((o, idx) => {
            const isActive = idx === activeIndex;
            const Icon = o.icon;
            return (
              <motion.div
                key={o.label}
                onClick={() => setActiveIndex(idx)}
                whileTap={{ scale: 0.97 }}
                className={`relative rounded-xl p-3 border transition-all duration-300 cursor-pointer overflow-hidden ${
                  isActive
                    ? "bg-white/[0.08] border-white/25 shadow-[0_0_20px_rgba(255,255,255,0.05)]"
                    : "bg-white/[0.02] border-white/5 opacity-60"
                }`}
              >
                {isActive && (
                  <div className="absolute inset-0 bg-gradient-to-br from-cyan/5 via-electric/5 to-purple/5 opacity-40 pointer-events-none" />
                )}
                <div className="relative flex flex-col items-center text-center">
                  <div
                    className={`p-2 rounded-lg transition-all duration-300 ${isActive ? "bg-white/10" : "bg-white/[0.02]"}`}
                  >
                    <Icon className={`h-5 w-5 ${isActive ? o.color : "text-white/40"}`} />
                  </div>
                  <div
                    className={`mt-2 font-display text-xs tracking-tight ${isActive ? "text-white font-medium" : "text-white/50"}`}
                  >
                    {o.label}
                  </div>
                  {isActive && (
                    <div className="mt-1.5 flex items-center gap-1 text-[8px] font-medium text-emerald uppercase tracking-wider">
                      <span className="h-1.5 w-1.5 rounded-full bg-emerald animate-pulse" />
                      <span>Running</span>
                    </div>
                  )}
                </div>
              </motion.div>
            );
          })}
        </div>
      </div>
    </Section>
  );
}
