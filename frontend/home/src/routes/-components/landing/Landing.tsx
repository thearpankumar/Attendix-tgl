import { DashboardShowcase } from "./DashboardShowcase";
import { EngineSection } from "./EngineSection";
import { FeaturesGrid } from "./FeaturesGrid";
import { FinalCTA } from "./FinalCTA";
import { Footer } from "./Footer";
import { Hero } from "./Hero";
import { Nav } from "./Nav";
import { CursorGlow } from "./primitives/CursorGlow";
import { ProblemSection } from "./ProblemSection";
import { SecurityVault } from "./SecurityVault";
import { Testimonials } from "./Testimonials";
import { useLenis } from "./useLenis";

/* ---------- Landing ---------- */
export function Landing() {
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
