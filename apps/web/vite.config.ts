import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const localRuntime = process.env.ORTYO_LOCAL_RUNTIME ?? "http://127.0.0.1:7777";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5173,
    proxy: {
      "/api": localRuntime,
      "/hook": localRuntime,
      "/claim": localRuntime,
      "/healthz": localRuntime,
      "/view": {
        target: localRuntime,
        ws: true,
      },
    },
  },
});
