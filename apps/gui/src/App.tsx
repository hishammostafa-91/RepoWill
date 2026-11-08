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
  const [detectResult, setDetectResult] = useState<string>("");
  const [projects, setProjects] = useState<string>("");
  const [initName, setInitName] = useState<string>("");
  const [initEmail, setInitEmail] = useState<string>("");
  const [initPath, setInitPath] = useState<string>(".");

  useEffect(() => {
    invoke<OpResult>("rw_op", { op: "app.version", params: null })
      .then((r) => setVersion(String(r.result)))
      .catch((e) => setError(String(e)));
  }, []);

  useEffect(() => {
    if (page === "projects") {
      invoke<OpResult>("rw_op", { op: "project.list", params: null })
        .then((r) => setProjects(JSON.stringify(r.result)))
        .catch((e) => setError(String(e)));
    }
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
        {page === "setup" && (
          <>
            <h1>Setup</h1>
            <div style={{ marginBottom: 8 }}>
              <input
                placeholder="path"
                value={initPath}
                onChange={(e) => setInitPath(e.target.value)}
              />
            </div>
            <button
              type="button"
              onClick={async () => {
                try {
                  const r = await invoke<OpResult>("rw_op", {
                    op: "project.detect",
                    params: { path: initPath },
                  });
                  setDetectResult(JSON.stringify(r.result));
                  setError("");
                } catch (e) {
                  setError(String(e));
                }
              }}
            >
              Detect
            </button>
            <button
              type="button"
              style={{ marginLeft: 8 }}
              onClick={async () => {
                try {
                  const r = await invoke<OpResult>("rw_op", {
                    op: "project.plan",
                    params: { path: initPath },
                  });
                  setDetectResult(JSON.stringify(r.result));
                  setError("");
                } catch (e) {
                  setError(String(e));
                }
              }}
            >
              Preview
            </button>
            <div style={{ marginTop: 8 }}>
              <input
                placeholder="name"
                value={initName}
                onChange={(e) => setInitName(e.target.value)}
              />
              <input
                placeholder="email"
                value={initEmail}
                onChange={(e) => setInitEmail(e.target.value)}
                style={{ marginLeft: 8 }}
              />
            </div>
            <button
              type="button"
              style={{ marginTop: 8 }}
              onClick={async () => {
                try {
                  const r = await invoke<OpResult>("rw_op", {
                    op: "project.init",
                    params: {
                      path: initPath,
                      name: initName,
                      email: initEmail,
                    },
                  });
                  setDetectResult(JSON.stringify(r.result));
                  setError("");
                } catch (e) {
                  setError(String(e));
                }
              }}
            >
              Apply
            </button>
            {detectResult && <pre>{detectResult}</pre>}
            {error && <p style={{ color: "red" }}>{error}</p>}
          </>
        )}
        {page === "projects" && (
          <>
            <h1>Projects</h1>
            <pre>{projects}</pre>
            {error && <p style={{ color: "red" }}>{error}</p>}
          </>
        )}
      </main>
    </div>
  );
}

export default App;
