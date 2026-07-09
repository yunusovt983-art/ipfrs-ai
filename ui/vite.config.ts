import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Relative base so the built bundle works under any sub-path
// (e.g. GitHub Pages /ipfrs-ai/ui/ or served standalone).
export default defineConfig({
  base: "./",
  plugins: [react()],
  server: { port: 5273, host: true },
});
