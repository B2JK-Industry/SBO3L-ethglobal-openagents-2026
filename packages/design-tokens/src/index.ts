// ESM-compatible re-exports — TypeScript preserves the `.js` literal in
// compiled output. Without `.js`, ESM resolution fails post-build with
// ERR_MODULE_NOT_FOUND. (Codex P2 on PR #98.)
export { tokens, darkTokens, lightTokens } from './tokens.js';
export type { Tokens } from './tokens.js';
