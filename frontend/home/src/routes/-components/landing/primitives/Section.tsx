import { motion } from "framer-motion";

/* ---------- Section shell ---------- */
export function Section({
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
