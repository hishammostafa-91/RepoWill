// SPDX-License-Identifier: AGPL-3.0-or-later

import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

interface OpResult {
  op: string;
  result: unknown;
}

function App() {
  const [version, setVersion] = useState<string>("…");
  const [error, setError] = useState<string>("");

  useEffect(() => {
    invoke<OpResult>("rw_op", { op: "app.version" })
      .then((r) => setVersion(String(r.result)))
      .catch((e) => setError(String(e)));
  }, []);

  return (
    <div style={{ display: "flex", height: "100vh", fontFamily: "sans-serif" }}>
      <aside style={{ width: 220, padding: 16, borderRight: "1px solid #ddd" }}>
        <h2>RepoWill</h2>
        <nav>
          <div>About</div>
        </nav>
      </aside>
      <main style={{ flex: 1, padding: 24 }}>
        <h1>About</h1>
        <p>RepoWill version: {version}</p>
        {error && <p style={{ color: "red" }}>{error}</p>}
      </main>
    </div>
  );
}

export default App;
