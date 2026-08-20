import { motion } from "framer-motion";

import { Section } from "./primitives/Section";

/* ---------- Section 11: Testimonials ---------- */
export function Testimonials() {
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
