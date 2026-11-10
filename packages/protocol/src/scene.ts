/**
 * SceneGraph snapshot returned by the engine's `scene` inspection request
 * (crates/brs-core/src/vm/debugger/rpc.rs). Used by the ECP app-ui query, the
 * scene console command and the web shell's node inspector.
 */
export interface SceneNode {
  type: string;
  /** Type chain, most derived first, ending with "Node". */
  ancestry: string[];
  focused: boolean;
  /** Field values as JSON (nodes and other objects are summarized). */
  fields: Record<string, unknown>;
  /** Scene-space rectangle in design coordinates: x, y, width, height. */
  bounds: number[];
  children: SceneNode[];
}

export interface SceneInspection {
  roots: SceneNode[];
  threads: number;
  timeMs: number;
  /** Design resolution of the channel (bounds are in this space). */
  width?: number;
  height?: number;
}
