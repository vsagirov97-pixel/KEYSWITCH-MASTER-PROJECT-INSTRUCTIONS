import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

// Placeholder shell. Real screens follow the approved design system (GATE 3).
export default function App() {
  const [coreVersion, setCoreVersion] = useState<string>("…");

  useEffect(() => {
    invoke<string>("core_version")
      .then(setCoreVersion)
      .catch(() => setCoreVersion("unavailable"));
  }, []);

  return (
    <main className="min-h-screen bg-neutral-950 p-8 font-sans text-neutral-100">
      <h1 className="text-xl font-semibold">KeySwitch</h1>
      <p className="mt-2 text-sm text-neutral-400">core {coreVersion}</p>
    </main>
  );
}
