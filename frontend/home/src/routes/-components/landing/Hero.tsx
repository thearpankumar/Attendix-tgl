import { motion, useScroll, useTransform } from "framer-motion";
import { ArrowRight, ChevronDown, Play } from "lucide-react";
import { useRef } from "react";

import { MagneticButton } from "./primitives/MagneticButton";
import { Particles } from "./primitives/Particles";

/* ---------- Hero ---------- */
export function Hero() {
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start start", "end start"] });
  const y = useTransform(scrollYProgress, [0, 1], [0, 120]);
  const opacity = useTransform(scrollYProgress, [0, 0.8], [1, 0]);

  return (
    <section ref={ref} id="top" className="relative min-h-[100svh] w-full overflow-hidden bg-mesh">
      <div className="absolute inset-0 grid-lines opacity-70" />
      <Particles count={60} />
      {/* animated glow orbs */}
      <div className="absolute -top-20 -left-40 h-[560px] w-[560px] rounded-full bg-electric/20 blur-3xl animate-drift" />
      <div
        className="absolute top-40 -right-40 h-[520px] w-[520px] rounded-full bg-purple/20 blur-3xl animate-drift"
        style={{ animationDelay: "-6s" }}
      />
      <div
        className="absolute bottom-0 left-1/3 h-[420px] w-[420px] rounded-full bg-cyan/15 blur-3xl animate-drift"
        style={{ animationDelay: "-3s" }}
      />

      <motion.div
        style={{ y, opacity }}
        className="relative z-10 max-w-4xl mx-auto px-6 pt-40 pb-24 flex flex-col items-center text-center min-h-[100svh] justify-center"
      >
        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8 }}
          className="inline-flex items-center gap-2 rounded-full glass px-3.5 py-1.5 text-xs text-white/80"
        >
          <span className="relative flex h-1.5 w-1.5">
            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald opacity-75" />
            <span className="relative inline-flex rounded-full h-1.5 w-1.5 bg-emerald" />
          </span>
          Live · Attendance Intelligence v4
        </motion.div>

        <motion.h1
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 1, delay: 0.1, ease: [0.22, 1, 0.36, 1] }}
          className="mt-6 font-display text-[clamp(3rem,8vw,7rem)] leading-[0.92] tracking-[-0.04em] font-medium"
        >
          <span className="text-gradient">Attendance.</span>
          <br />
          <span className="text-gradient-accent">Verified.</span>
          <br />
          <span className="text-white/40">Not assumed.</span>
        </motion.h1>

        <motion.p
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.3 }}
          className="mt-8 max-w-xl text-lg text-white/60 leading-relaxed"
        >
          AI-powered attendance verification using face recognition, biometrics, passkeys and
          geo-location. Built for enterprises that can't afford to guess.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, delay: 0.45 }}
          className="mt-10 flex flex-wrap items-center justify-center gap-3"
        >
          <MagneticButton href="/mark-attendance" primary>
            Mentor login <ArrowRight className="h-4 w-4" />
          </MagneticButton>
          <MagneticButton href="#platform">
            <Play className="h-3.5 w-3.5" /> Watch platform
          </MagneticButton>
        </motion.div>

        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 1, delay: 0.8 }}
          className="mt-14 flex flex-wrap items-center justify-center gap-x-8 gap-y-3 text-xs uppercase tracking-[0.2em] text-white/40"
        >
          <span>SOC 2 Ready</span>
          <span>·</span>
          <span>GDPR</span>
          <span>·</span>
          <span>ISO 27001</span>
          <span>·</span>
          <span>Zero Trust</span>
        </motion.div>
      </motion.div>

      {/* scroll indicator */}
      <motion.div
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ delay: 1.5 }}
        className="absolute bottom-8 left-1/2 -translate-x-1/2 flex flex-col items-center gap-2 text-white/40"
      >
        <span className="text-[10px] uppercase tracking-[0.3em]">Scroll</span>
        <motion.div animate={{ y: [0, 6, 0] }} transition={{ duration: 1.6, repeat: Infinity }}>
          <ChevronDown className="h-4 w-4" />
        </motion.div>
      </motion.div>
    </section>
  );
}
