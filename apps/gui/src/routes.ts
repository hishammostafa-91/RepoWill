// SPDX-License-Identifier: AGPL-3.0-or-later

export type Route = "about" | "policy";

export const routes: { id: Route; label: string; path: string }[] = [
  { id: "about", label: "About", path: "/settings/about" },
  { id: "policy", label: "Policy viewer", path: "/developer/policy" },
];
