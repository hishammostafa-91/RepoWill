// SPDX-License-Identifier: AGPL-3.0-or-later

export type Route = "about" | "policy" | "setup" | "projects";

export const routes: { id: Route; label: string; path: string }[] = [
  { id: "about", label: "About", path: "/settings/about" },
  { id: "policy", label: "Policy viewer", path: "/developer/policy" },
  { id: "setup", label: "Setup", path: "/setup" },
  { id: "projects", label: "Projects", path: "/projects" },
];
