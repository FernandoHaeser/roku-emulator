// shaka-player ships global namespace typings, which only a triple-slash
// reference can pull in without turning this file into a module.
// eslint-disable-next-line @typescript-eslint/triple-slash-reference
/// <reference path="../../node_modules/shaka-player/dist/shaka-player.compiled.d.ts" />

// Expose the global namespace as the module's default export.
declare module "shaka-player/dist/shaka-player.compiled.js" {
  export default shaka;
}

// mux.js (TS -> fMP4 transmuxing for Shaka) has no typings; Shaka only needs
// it on `window.muxjs`.
declare module "mux.js" {
  const muxjs: unknown;
  export default muxjs;
}
