import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    port: 1420,
    strictPort: true,
    // src-tauri/fixtures doubles as a dev-time video library folder --
    // without this, dropping/removing a video there (what a user does
    // constantly while picking wallpapers) gets picked up as a source
    // change and triggers a full page reload, wiping all React state.
    watch: {
      ignored: ["**/src-tauri/fixtures/**"],
    },
  },
});
