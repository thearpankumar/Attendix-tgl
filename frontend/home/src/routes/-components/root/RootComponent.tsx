import { QueryClientProvider } from "@tanstack/react-query";
import { Outlet } from "@tanstack/react-router";

import { Route } from "../../__root";

export function RootComponent() {
  const { queryClient } = Route.useRouteContext();
  return (
    <QueryClientProvider client={queryClient}>
      <Outlet />
    </QueryClientProvider>
  );
}
