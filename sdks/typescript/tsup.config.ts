import { defineConfig } from "tsup";

export default defineConfig({
  entry: {
    index: "src/index.ts",
    passport: "src/passport.ts",
    auth: "src/auth.ts",
    types: "src/types.ts",
  },
  format: ["esm", "cjs"],
  dts: true,
  splitting: false,
  sourcemap: true,
  clean: true,
  treeshake: true,
  target: "node18",
  outDir: "dist",
  minify: false,
});
