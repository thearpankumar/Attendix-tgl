import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import { motion, useScroll, useTransform, useSpring, useMotionValue } from "framer-motion";
import {
  ScanFace,
  Fingerprint,
  MapPin,
  KeyRound,
  Cpu,
  ShieldCheck,
  Activity,
  AlertTriangle,
  ArrowRight,
  Play,
  Check,
  X,
  Sparkles,
  ChevronDown,
  LineChart,
  Users,
  Globe,
  Lock,
  Eye,
  type LucideIcon,
} from "lucide-react";

export const Route = createFileRoute("/")({
  component: Landing,
  head: () => ({
    meta: [
      {
        property: "og:image",
        content:
          "https://images.unsplash.com/photo-1451187580459-43490279c0fa?w=1200&h=630&fit=crop",
      },
    ],
  }),
});

/* ---------- Smooth scroll ---------- */
function useLenis() {
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

/* ---------- Cursor glow ---------- */
function CursorGlow() {
  const x = useMotionValue(-200);
  const y = useMotionValue(-200);
  const sx = useSpring(x, { stiffness: 200, damping: 30 });
  const sy = useSpring(y, { stiffness: 200, damping: 30 });
  useEffect(() => {
    const move = (e: MouseEvent) => {
      x.set(e.clientX);
      y.set(e.clientY);
    };
    window.addEventListener("mousemove", move);
    return () => window.removeEventListener("mousemove", move);
  }, [x, y]);
  return (
    <motion.div
      aria-hidden
      className="pointer-events-none fixed z-[100] hidden md:block"
      style={{
        x: sx,
        y: sy,
        translateX: "-50%",
        translateY: "-50%",
        width: 480,
        height: 480,
        background:
          "radial-gradient(circle, rgba(61,139,255,0.14), rgba(167,139,250,0.06) 40%, transparent 70%)",
        filter: "blur(40px)",
        mixBlendMode: "screen",
      }}
    />
  );
}

/* ---------- Nav ---------- */
function Nav() {
  const [scrolled, setScrolled] = useState(false);
  useEffect(() => {
    const onScroll = () => setScrolled(window.scrollY > 20);
    window.addEventListener("scroll", onScroll);
    return () => window.removeEventListener("scroll", onScroll);
  }, []);
  return (
    <motion.header
      initial={{ y: -40, opacity: 0 }}
      animate={{ y: 0, opacity: 1 }}
      transition={{ duration: 0.8, ease: [0.22, 1, 0.36, 1] }}
      className="fixed top-4 left-1/2 -translate-x-1/2 z-50 w-[min(1120px,calc(100%-2rem))]"
    >
      <div
        className={`glass rounded-full pl-5 pr-5 py-2 flex items-center justify-between transition-all duration-500 ${scrolled ? "backdrop-blur-2xl" : ""}`}
      >
        <a href="#top" className="flex items-center gap-2">
          <div className="relative h-7 w-7 rounded-lg bg-gradient-to-br from-cyan via-electric to-purple grid place-items-center">
            <div className="absolute inset-[3px] rounded-md bg-[#06070A] grid place-items-center">
              <span className="text-[10px] font-bold tracking-tighter text-gradient-accent">
                AX
              </span>
            </div>
          </div>
          <span className="font-display font-semibold tracking-tight">Attendix</span>
          <span className="hidden sm:inline text-xs text-muted-foreground ml-1">
            by TalenciaGlobal
          </span>
        </a>
        <nav className="hidden md:flex items-center gap-7 text-sm text-white/70">
          {["Platform", "Security", "Industries", "Enterprise", "Pricing"].map((l) => (
            <a key={l} href={`#${l.toLowerCase()}`} className="hover:text-white transition-colors">
              {l}
            </a>
          ))}
        </nav>
      </div>
    </motion.header>
  );
}

/* ---------- Particles ---------- */
interface Particle {
  id: number;
  size: number;
  dur: number;
  left: number;
  top: number;
  delay: number;
}

function Particles({ count = 40 }: { count?: number }) {
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

/* ---------- Hero ---------- */
function Hero() {
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

/* ---------- Magnetic Button ---------- */
function MagneticButton({
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

/* ---------- Section shell ---------- */
function Section({
  id,
  eyebrow,
  title,
  subtitle,
  children,
  className = "",
}: {
  id?: string;
  eyebrow?: string;
  title?: React.ReactNode;
  subtitle?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section id={id} className={`relative py-32 px-6 ${className}`}>
      <div className="max-w-7xl mx-auto">
        {(eyebrow || title) && (
          <motion.div
            initial={{ opacity: 0, y: 30 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true, margin: "-100px" }}
            transition={{ duration: 0.8, ease: [0.22, 1, 0.36, 1] }}
            className="mb-16 max-w-3xl"
          >
            {eyebrow && (
              <div className="text-xs uppercase tracking-[0.3em] text-white/40 mb-4">{eyebrow}</div>
            )}
            {title && (
              <h2 className="font-display text-[clamp(2rem,5vw,4.5rem)] leading-[0.95] tracking-[-0.03em] text-gradient">
                {title}
              </h2>
            )}
            {subtitle && <p className="mt-6 text-lg text-white/60 leading-relaxed">{subtitle}</p>}
          </motion.div>
        )}
        {children}
      </div>
    </section>
  );
}

/* ---------- Section 2: Problem ---------- */
function ProblemSection() {
  const old = [
    { name: "Punch Cards", flaw: "Buddy punching" },
    { name: "RFID", flaw: "Card sharing" },
    { name: "PIN codes", flaw: "Guessable" },
    { name: "OTP", flaw: "Phone forwarding" },
    { name: "Manual sheets", flaw: "Human error" },
    { name: "Excel logs", flaw: "Editable, unaudited" },
    { name: "GPS-only", flaw: "Spoof-able" },
  ];
  return (
    <Section
      id="problem"
      eyebrow="The problem"
      title={
        <>
          Attendance has been <span className="text-gradient-accent">broken</span> for years.
        </>
      }
      subtitle="Every legacy method quietly leaks payroll, hours, and trust. It's not a policy problem — it's a verification problem."
    >
      <div className="grid md:grid-cols-2 lg:grid-cols-4 gap-4">
        {old.map((m, i) => (
          <motion.div
            key={m.name}
            initial={{ opacity: 0, y: 30, filter: "blur(8px)" }}
            whileInView={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            viewport={{ once: true }}
            transition={{ duration: 0.6, delay: i * 0.06 }}
            whileHover={{ y: -6, rotate: -0.5 }}
            className="group relative glass rounded-2xl p-5 overflow-hidden"
          >
            <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition bg-gradient-to-br from-destructive/10 to-transparent" />
            <div className="relative flex items-start justify-between">
              <div>
                <div className="text-xs uppercase tracking-widest text-white/40">Legacy</div>
                <div className="mt-1 font-display text-xl">{m.name}</div>
              </div>
              <span className="text-destructive/70">
                <X className="h-4 w-4" />
              </span>
            </div>
            <div className="relative mt-6 text-xs text-white/50 flex items-center gap-1.5">
              <AlertTriangle className="h-3 w-3 text-destructive/80" /> {m.flaw}
            </div>
            {/* crack */}
            <svg
              className="absolute -bottom-2 -right-2 opacity-30 group-hover:opacity-60 transition"
              width="80"
              height="80"
              viewBox="0 0 80 80"
            >
              <path
                d="M10 70 L30 40 L20 30 L45 10"
                stroke="rgba(255,80,80,0.6)"
                strokeWidth="1"
                fill="none"
              />
              <path
                d="M30 40 L50 45 L60 30"
                stroke="rgba(255,80,80,0.4)"
                strokeWidth="1"
                fill="none"
              />
            </svg>
          </motion.div>
        ))}
        <motion.div
          initial={{ opacity: 0, scale: 0.9 }}
          whileInView={{ opacity: 1, scale: 1 }}
          viewport={{ once: true }}
          transition={{ duration: 0.8, delay: 0.5 }}
          className="relative rounded-2xl p-5 overflow-hidden bg-gradient-to-br from-cyan/20 via-electric/20 to-purple/20 border border-white/20"
        >
          <div className="absolute inset-0 bg-[#06070A]/60 backdrop-blur-xl" />
          <div className="relative">
            <div className="text-xs uppercase tracking-widest text-cyan">The shift</div>
            <div className="mt-1 font-display text-xl">Attendix</div>
            <div className="mt-6 text-xs text-emerald flex items-center gap-1.5">
              <Check className="h-3 w-3" /> Verified identity
            </div>
          </div>
          <div className="absolute -inset-8 opacity-40 blur-2xl bg-gradient-to-br from-cyan via-electric to-purple pointer-events-none" />
        </motion.div>
      </div>
    </Section>
  );
}

/* ---------- Section 3: Engine (orbit) ---------- */
function EngineSection() {
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

/* ---------- Section 5: Features ---------- */
function FeaturesGrid() {
  const feats = [
    {
      icon: ScanFace,
      title: "Face Recognition",
      desc: "3D depth-mapped identity match with anti-spoof.",
      size: "lg:col-span-2 lg:row-span-2",
    },
    { icon: Fingerprint, title: "Biometrics", desc: "Native TouchID, FaceID and Windows Hello." },
    { icon: MapPin, title: "Geo-location", desc: "Geofence + IP + WiFi triangulation." },
    { icon: KeyRound, title: "Passkeys", desc: "FIDO2 signed check-ins. Phish-proof." },
    {
      icon: LineChart,
      title: "Workforce Analytics",
      desc: "Live dashboards, cohort trends, exports.",
      size: "lg:col-span-2",
    },
    { icon: Activity, title: "Live Monitoring", desc: "Real-time attendance & anomaly stream." },
    { icon: AlertTriangle, title: "Fraud Alerts", desc: "23-signal scoring, tuned per site." },
    { icon: ShieldCheck, title: "Admin Console", desc: "Roles, audit trail, SSO, SCIM." },
  ];
  return (
    <Section
      id="features"
      eyebrow="Platform"
      title={
        <>
          Everything the <span className="text-gradient-accent">modern workforce</span> needs.
        </>
      }
    >
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 auto-rows-[220px]">
        {feats.map((f, i) => (
          <TiltCard key={f.title} className={f.size}>
            <motion.div
              initial={{ opacity: 0, y: 20 }}
              whileInView={{ opacity: 1, y: 0 }}
              viewport={{ once: true }}
              transition={{ delay: i * 0.05, duration: 0.6 }}
              className="relative h-full w-full glass rounded-2xl p-6 overflow-hidden group"
            >
              <div className="absolute -inset-px rounded-2xl opacity-0 group-hover:opacity-100 transition duration-500 bg-gradient-to-br from-cyan/10 via-transparent to-purple/10 pointer-events-none" />
              <div className="relative flex items-start justify-between">
                <div className="h-11 w-11 rounded-xl glass-strong grid place-items-center">
                  <f.icon className="h-5 w-5 text-cyan" />
                </div>
                <ArrowRight className="h-4 w-4 text-white/30 group-hover:text-white group-hover:translate-x-1 transition" />
              </div>
              <div className="absolute bottom-6 left-6 right-6">
                <div className="font-display text-xl">{f.title}</div>
                <div className="mt-1.5 text-sm text-white/55">{f.desc}</div>
              </div>
            </motion.div>
          </TiltCard>
        ))}
      </div>
    </Section>
  );
}

function TiltCard({ children, className = "" }: { children: React.ReactNode; className?: string }) {
  const ref = useRef<HTMLDivElement>(null);
  const rx = useMotionValue(0);
  const ry = useMotionValue(0);
  return (
    <div
      ref={ref}
      className={className}
      onMouseMove={(e) => {
        const r = ref.current!.getBoundingClientRect();
        const px = (e.clientX - r.left) / r.width - 0.5;
        const py = (e.clientY - r.top) / r.height - 0.5;
        ry.set(px * 8);
        rx.set(-py * 8);
      }}
      onMouseLeave={() => {
        rx.set(0);
        ry.set(0);
      }}
      style={{ perspective: 1200 }}
    >
      <motion.div
        style={{ rotateX: rx, rotateY: ry, transformStyle: "preserve-3d" }}
        className="h-full w-full"
      >
        {children}
      </motion.div>
    </div>
  );
}

/* ---------- Section 6: Interactive Dashboard ---------- */
function DashboardShowcase() {
  const ref = useRef<HTMLDivElement>(null);
  const { scrollYProgress } = useScroll({ target: ref, offset: ["start end", "end start"] });
  const rotate = useTransform(scrollYProgress, [0, 0.5, 1], [8, 0, -6]);
  const scale = useTransform(scrollYProgress, [0, 0.5], [0.92, 1]);
  return (
    <Section
      id="dashboard"
      eyebrow="Live console"
      title={
        <>
          The command center for <span className="text-gradient-accent">every shift.</span>
        </>
      }
      subtitle="A workspace built for HR, security and operations — with real-time verification, maps and audit-grade logs."
    >
      <motion.div
        ref={ref}
        style={{ rotateX: rotate, scale, transformPerspective: 1400 }}
        className="relative rounded-3xl glass-strong overflow-hidden noise"
      >
        <div className="absolute inset-0 bg-gradient-to-br from-cyan/5 via-transparent to-purple/5 pointer-events-none" />
        <div className="grid grid-cols-1 md:grid-cols-[220px,1fr]">
          {/* sidebar */}
          <aside className="flex flex-row md:flex-col overflow-x-auto md:overflow-x-visible border-b md:border-b-0 md:border-r border-white/10 p-3 md:p-4 gap-1.5 md:space-y-1 [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
            <div className="hidden md:block text-[10px] uppercase tracking-widest text-white/40 mb-3 px-2">
              Workspace
            </div>
            {(
              [
                ["Live", Activity, true],
                ["People", Users],
                ["Sites", Globe],
                ["Verifications", ShieldCheck],
                ["Analytics", LineChart],
                ["Alerts", AlertTriangle],
                ["Settings", Cpu],
              ] as [string, LucideIcon, boolean?][]
            ).map(([label, Icon, active]) => (
              <div
                key={label}
                className={`flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-sm shrink-0 ${active ? "bg-white/10 text-white" : "text-white/50 hover:text-white/80"}`}
              >
                <Icon className="h-3.5 w-3.5" /> {label}
              </div>
            ))}
          </aside>
          {/* main */}
          <div className="p-4 sm:p-5">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
              <div>
                <div className="text-[11px] uppercase tracking-widest text-white/40">
                  Today · Global
                </div>
                <div className="font-display text-xl sm:text-2xl">Live attendance</div>
              </div>
              <div className="flex items-center gap-2 text-xs self-start sm:self-auto">
                <span className="rounded-full glass px-2.5 py-1 flex items-center gap-1.5">
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald animate-pulse" /> streaming
                </span>
                <span className="rounded-full glass px-2.5 py-1">Last 24h</span>
              </div>
            </div>
            <div className="mt-5 grid grid-cols-2 sm:grid-cols-4 gap-3">
              {[
                { k: "Verified", v: "12,483", d: "+4.2%", c: "text-emerald" },
                { k: "In progress", v: "127", d: "live", c: "text-cyan" },
                { k: "Flagged", v: "3", d: "review", c: "text-destructive" },
                { k: "Sites", v: "218", d: "online", c: "text-purple" },
              ].map((s) => (
                <div key={s.k} className="rounded-xl glass p-3">
                  <div className="text-[9px] sm:text-[10px] uppercase tracking-wider sm:tracking-widest text-white/40">
                    {s.k}
                  </div>
                  <div className="mt-1 font-display text-xl sm:text-2xl tabular-nums">{s.v}</div>
                  <div className={`text-[11px] ${s.c}`}>{s.d}</div>
                </div>
              ))}
            </div>
            <div className="mt-4 grid grid-cols-1 lg:grid-cols-3 gap-3">
              <div className="col-span-1 lg:col-span-2 rounded-xl glass p-4 h-52 relative overflow-hidden">
                <div className="text-[11px] uppercase tracking-widest text-white/40">
                  Verification volume
                </div>
                <svg
                  viewBox="0 0 400 140"
                  className="absolute inset-0 top-8 h-[calc(100%-2rem)] w-full"
                >
                  <defs>
                    <linearGradient id="g1" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#22D3EE" stopOpacity="0.5" />
                      <stop offset="100%" stopColor="#22D3EE" stopOpacity="0" />
                    </linearGradient>
                  </defs>
                  <path
                    d="M0 100 C 40 80, 80 90, 120 60 S 200 30, 260 50 S 340 20, 400 10 L 400 140 L 0 140 Z"
                    fill="url(#g1)"
                  />
                  <path
                    d="M0 100 C 40 80, 80 90, 120 60 S 200 30, 260 50 S 340 20, 400 10"
                    stroke="#22D3EE"
                    strokeWidth="1.5"
                    fill="none"
                  />
                </svg>
              </div>
              <div className="rounded-xl glass p-4 h-52 overflow-y-auto [&::-webkit-scrollbar]:hidden [-ms-overflow-style:none] [scrollbar-width:none]">
                <div className="text-[11px] uppercase tracking-widest text-white/40">
                  Face match log
                </div>
                <div className="mt-3 space-y-2">
                  {[
                    "A. Nadella · 99.98%",
                    "S. Pichai · 99.94%",
                    "T. Cook · 99.91%",
                    "J. Huang · 99.97%",
                    "L. Su · 99.93%",
                  ].map((n, i) => (
                    <div key={i} className="flex items-center justify-between text-[11px]">
                      <span className="flex items-center gap-2 text-white/80">
                        <span className="h-5 w-5 rounded-full bg-gradient-to-br from-cyan to-purple shrink-0" />
                        <span className="truncate">{n.split(" · ")[0]}</span>
                      </span>
                      <span className="text-emerald tabular-nums shrink-0">
                        {n.split(" · ")[1]}
                      </span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      </motion.div>
    </Section>
  );
}

/* ---------- Section 10: Security vault ---------- */
function SecurityVault() {
  return (
    <Section
      id="security"
      eyebrow="Security"
      title={
        <>
          Enter the <span className="text-gradient-accent">vault.</span>
        </>
      }
      subtitle="Zero-trust by design. Every verification is signed, timestamped and stored in a tamper-evident ledger."
    >
      <div className="relative grid lg:grid-cols-2 gap-8 items-center">
        <div className="relative aspect-square max-w-[520px] mx-auto w-full">
          <motion.div
            animate={{ rotate: 360 }}
            transition={{ duration: 40, repeat: Infinity, ease: "linear" }}
            className="absolute inset-0 rounded-full border border-white/[0.08]"
          />
          <motion.div
            animate={{ rotate: -360 }}
            transition={{ duration: 50, repeat: Infinity, ease: "linear" }}
            className="absolute inset-8 rounded-full border border-dashed border-white/[0.06]"
          />
          <motion.div
            animate={{ scale: [1, 1.04, 1] }}
            transition={{ duration: 4, repeat: Infinity }}
            className="absolute inset-16 rounded-full glass-strong grid place-items-center overflow-hidden"
          >
            <div className="absolute inset-0 bg-gradient-to-br from-electric/25 via-purple/10 to-cyan/25 blur-2xl" />
            <ShieldCheck className="relative h-24 w-24 text-white/90" strokeWidth={1.2} />
          </motion.div>
          {["SOC 2", "GDPR", "ISO 27001", "HIPAA", "FIDO2", "Zero Trust"].map((b, i) => {
            const a = (i / 6) * Math.PI * 2;
            const x = 50 + Math.cos(a) * 46;
            const y = 50 + Math.sin(a) * 46;
            return (
              <div
                key={b}
                className="absolute -translate-x-1/2 -translate-y-1/2"
                style={{ left: `${x}%`, top: `${y}%` }}
              >
                <div className="glass rounded-full px-3 py-1 text-[11px] tracking-wider">{b}</div>
              </div>
            );
          })}
        </div>
        <div className="space-y-3">
          {[
            {
              icon: Lock,
              t: "Encrypted end-to-end",
              d: "AES-256 at rest · TLS 1.3 in flight · HSM-backed keys.",
            },
            {
              icon: KeyRound,
              t: "Passkeys & FIDO2",
              d: "Phish-resistant auth, hardware-bound credentials.",
            },
            {
              icon: Eye,
              t: "Liveness & anti-spoof",
              d: "Depth, motion and reflection checks reject video replay.",
            },
            {
              icon: Cpu,
              t: "AI fraud engine",
              d: "23-signal scoring, tuned per site, learns weekly.",
            },
            {
              icon: ShieldCheck,
              t: "Tamper-evident ledger",
              d: "Every event is hash-chained and exportable.",
            },
          ].map((x) => (
            <div key={x.t} className="glass rounded-xl p-4 flex gap-4">
              <div className="h-10 w-10 rounded-lg glass-strong grid place-items-center shrink-0">
                <x.icon className="h-4 w-4 text-cyan" />
              </div>
              <div>
                <div className="font-medium">{x.t}</div>
                <div className="text-sm text-white/55 mt-0.5">{x.d}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </Section>
  );
}

/* ---------- Section 11: Testimonials ---------- */
function Testimonials() {
  const items = [
    {
      q: "We eliminated buddy punching in 3 weeks. Payroll fraud dropped to zero.",
      a: "VP People Ops, Fortune 500 manufacturer",
    },
    {
      q: "The verification console is what our security team dreamed of for a decade.",
      a: "CISO, National healthcare network",
    },
    {
      q: "Deployed across 214 sites with SSO in under a month. It just works.",
      a: "CTO, Global construction group",
    },
    {
      q: "Attendix is the first attendance product that ever felt like software.",
      a: "COO, EdTech unicorn",
    },
    {
      q: "Cryptographically signed logs made our compliance audit painless.",
      a: "GRC Lead, Financial services",
    },
  ];
  return (
    <Section
      id="testimonials"
      eyebrow="Trusted"
      title={
        <>
          Teams that <span className="text-gradient-accent">rely on it.</span>
        </>
      }
    >
      <div className="relative overflow-hidden">
        <div className="absolute left-0 top-0 bottom-0 w-24 z-10 bg-gradient-to-r from-[#06070A] to-transparent pointer-events-none" />
        <div className="absolute right-0 top-0 bottom-0 w-24 z-10 bg-gradient-to-l from-[#06070A] to-transparent pointer-events-none" />
        <motion.div
          animate={{ x: ["0%", "-50%"] }}
          transition={{ duration: 40, repeat: Infinity, ease: "linear" }}
          className="flex gap-4 w-max"
        >
          {[...items, ...items].map((t, i) => (
            <div key={i} className="w-[380px] shrink-0 glass rounded-2xl p-6">
              <div className="text-cyan text-3xl leading-none">"</div>
              <p className="mt-2 text-white/85 leading-relaxed">{t.q}</p>
              <div className="mt-4 text-xs text-white/40 uppercase tracking-widest">{t.a}</div>
            </div>
          ))}
        </motion.div>
      </div>
    </Section>
  );
}

/* ---------- Section 13: Final CTA ---------- */
function FinalCTA() {
  return (
    <section id="demo" className="relative py-40 px-6 overflow-hidden">
      <div className="absolute inset-0 bg-mesh opacity-90" />
      <div className="absolute inset-0 grid-lines opacity-40" />
      <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 h-[800px] w-[800px] rounded-full bg-gradient-to-br from-cyan/20 via-electric/15 to-purple/25 blur-3xl animate-pulse-glow" />
      <Particles count={40} />
      <div className="relative max-w-5xl mx-auto text-center">
        <motion.div
          initial={{ opacity: 0, y: 40 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          transition={{ duration: 1, ease: [0.22, 1, 0.36, 1] }}
        >
          <div className="text-xs uppercase tracking-[0.3em] text-white/40">
            The future of attendance
          </div>
          <h2 className="mt-6 font-display text-[clamp(2.5rem,7vw,6rem)] leading-[0.95] tracking-[-0.04em]">
            <span className="text-gradient">Attendance you can trust.</span>
            <br />
            <span className="text-gradient-accent">Every single time.</span>
          </h2>
          <p className="mt-8 max-w-xl mx-auto text-white/60">
            See Attendix live with your data. Enterprise pilots start in as little as one week.
          </p>
          <div className="mt-10 flex flex-wrap items-center justify-center gap-3">
            <MagneticButton href="mailto:hello@talenciaglobal.com" primary>
              Book a demo <ArrowRight className="h-4 w-4" />
            </MagneticButton>
            <MagneticButton href="mailto:sales@talenciaglobal.com">
              Request enterprise pricing
            </MagneticButton>
          </div>
        </motion.div>
      </div>
    </section>
  );
}

/* ---------- Footer ---------- */
function Footer() {
  return (
    <footer className="relative border-t border-white/10 bg-[#04050890]">
      <div className="max-w-7xl mx-auto px-6 py-10 flex flex-col sm:flex-row items-center justify-between gap-4 text-center sm:text-left">
        <div className="flex items-center gap-2">
          <div className="relative h-7 w-7 rounded-lg bg-gradient-to-br from-cyan via-electric to-purple grid place-items-center">
            <div className="absolute inset-[3px] rounded-md bg-[#06070A] grid place-items-center">
              <span className="text-[10px] font-bold text-gradient-accent">AX</span>
            </div>
          </div>
          <div className="leading-tight">
            <div className="font-display font-semibold">Attendix</div>
            <div className="text-xs text-white/40">by TalenciaGlobal</div>
          </div>
        </div>
        <a
          href="mailto:hello@talenciaglobal.com"
          className="text-sm text-white/60 hover:text-white transition"
        >
          hello@talenciaglobal.com
        </a>
        <div className="text-xs text-white/40">
          © {new Date().getFullYear()} TalenciaGlobal. All rights reserved.
        </div>
      </div>
    </footer>
  );
}

/* ---------- Landing ---------- */
function Landing() {
  useLenis();
  return (
    <main className="relative bg-[#06070A] text-white">
      <CursorGlow />
      <Nav />
      <Hero />
      <ProblemSection />
      <EngineSection />
      <FeaturesGrid />
      <DashboardShowcase />
      <SecurityVault />
      <Testimonials />
      <FinalCTA />
      <Footer />
    </main>
  );
}
