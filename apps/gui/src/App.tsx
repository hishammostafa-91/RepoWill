// SPDX-License-Identifier: AGPL-3.0-or-later

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { type Route, routes } from "./routes";

interface OpResult {
  op: string;
  result: unknown;
}

interface Diagnostic {
  code: string;
  severity: string;
  message: string;
  path?: string | null;
  line?: number | null;
  column?: number | null;
  fix?: string | null;
}

function App() {
  const [page, setPage] = useState<Route>("about");
  const [version, setVersion] = useState<string>("…");
  const [error, setError] = useState<string>("");
  const [policy, setPolicy] = useState<string>("");
  const [diagnostics, setDiagnostics] = useState<Diagnostic[]>([]);

  useEffect(() => {
    invoke<OpResult>("rw_op", { op: "app.version", params: null })
      .then((r) => setVersion(String(r.result)))
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (page === "policy") {
      invoke<OpResult>("rw_op", { op: "policy.show", params: null })
        .then((r) => {
          const result = r.result as { policy?: string };
          setPolicy(result.policy ?? "");
        })
        .catch((e) => setError(String(e)));
    }
  }, [page]);

  const validate = async () => {
    try {
      const r = await invoke<OpResult>("rw_op", {
        op: "policy.validate",
        params: { source: policy },
      });
      const result = r.result as { diagnostics?: Diagnostic[] };
      setDiagnostics(result.diagnostics ?? []);
      setError("");
    } catch (e) {
      const text = String(e);
      try {
        const parsed = JSON.parse(text) as {
          details?: { diagnostics?: Diagnostic[] };
        };
        setDiagnostics(parsed.details?.diagnostics ?? []);
      } catch {
        setDiagnostics([]);
      }
      setError(text);
    }
  };

  return (
    <div style={{ display: "flex", height: "100vh", fontFamily: "sans-serif" }}>
      <aside style={{ width: 220, padding: 16, borderRight: "1px solid #ddd" }}>
        <h2>RepoWill</h2>
        <nav style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {routes.map((route) => (
            <button
              type="button"
              key={route.id}
              onClick={() => setPage(route.id)}
            >
              {route.label}
            </button>
          ))}
        </nav>
      </aside>
      <main style={{ flex: 1, padding: 24, overflow: "auto" }}>
        {page === "about" && (
          <>
            <h1>About</h1>
            <p>RepoWill version: {version}</p>
            {error && <p style={{ color: "red" }}>{error}</p>}
          </>
        )}
        {page === "policy" && (
          <>
            <h1>Policy viewer</h1>
            <textarea
              value={policy}
              onChange={(e) => setPolicy(e.target.value)}
              style={{ width: "100%", height: 320, fontFamily: "monospace" }}
            />
            <div style={{ marginTop: 8 }}>
              <button type="button" onClick={validate}>
                Validate
              </button>
            </div>
            {diagnostics.length > 0 && (
              <ul>
                {diagnostics.map((diagnostic, index) => (
                  <li key={`${diagnostic.code}-${diagnostic.line ?? index}`}>
                    {diagnostic.code} {diagnostic.severity}:{" "}
                    {diagnostic.message}
                    {diagnostic.line ? ` (line ${diagnostic.line})` : ""}
                    {diagnostic.fix ? ` - ${diagnostic.fix}` : ""}
                  </li>
                ))}
              </ul>
            )}
            {error && <p style={{ color: "red" }}>{error}</p>}
          </>
        )}
      </main>
    </div>
  );
}

export default App;
