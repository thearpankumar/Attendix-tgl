import { motion, useScroll, useTransform } from "framer-motion";
import {
  Activity,
  AlertTriangle,
  Cpu,
  Globe,
  LineChart,
  ShieldCheck,
  Users,
  type LucideIcon,
} from "lucide-react";
import { useRef } from "react";

import { Section } from "./primitives/Section";

/* ---------- Section 6: Interactive Dashboard ---------- */
export function DashboardShowcase() {
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
