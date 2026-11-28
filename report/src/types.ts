import type { ComponentType } from "react";

export type View = "chart" | "table";
export type Theme = "system" | "light" | "dark";

export interface RenderContext {
  view: View;
  theme: Theme;
  data: unknown;
  components: { get(id: string): ComponentType<Record<string, unknown>> };
}

export interface Module {
  id: string;
  version: string;
  entry: string;
  schema: object | boolean;
}

export interface CatalogComponent extends Module {
  description?: string;
  example: string;
}

export interface Job {
  protocol: string;
  workDir: string;
  entry: string;
  data: string;
  renderer: Module;
  components: Module[];
  view: View | "fragment";
  theme: Theme;
  stage: string;
  artifact: string;
  fragment?: {
    width: number;
    height: number;
    components: { entry: string; x: number; y: number; width: number; height: number }[];
  };
  catalog?: CatalogComponent[];
}
