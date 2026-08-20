import { motion } from "framer-motion";
import { AlertTriangle, Check, X } from "lucide-react";

import { Section } from "./primitives/Section";

/* ---------- Section 2: Problem ---------- */
export function ProblemSection() {
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
