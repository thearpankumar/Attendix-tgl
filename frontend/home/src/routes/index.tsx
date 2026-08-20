import { createFileRoute } from "@tanstack/react-router";

import { Landing } from "./-components/landing/Landing";

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
