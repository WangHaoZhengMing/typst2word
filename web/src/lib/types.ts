export interface UnsupportedElement {
  element_name: string;
  reason: string;
  source_location: string | null;
}

export interface ConversionReport {
  pages_count: number;
  headings_count: number;
  paragraphs_count: number;
  lists_count: number;
  tables_count: number;
  equations_count: number;
  shapes_count: number;
  cetz_canvases_count: number;
  images_count: number;
  quotes_count: number;
  code_blocks_count: number;
  figures_count: number;
  footnotes_count: number;
  links_count: number;
  unsupported_elements: UnsupportedElement[];
  compiler_warnings: string[];
  font_fallback_warnings?: string[];
}

export interface MissingPackageDto {
  spec: string;
  namespace: string;
  name: string;
  version: string;
  download_url: string;
}

export interface PreviewResponseDto {
  status: 'ok' | 'missing_packages' | 'error';
  page_count: number;
  pages_svg: string[];
  compiler_warnings: string[];
  report: ConversionReport | null;
  document_title: string | null;
  missing_packages: MissingPackageDto[];
  error: string | null;
}

export interface FontFamilyDto {
  family: string;
  is_system_installed: boolean;
}

export interface OpenXmlFileEntryDto {
  path: string;
  size_bytes: number;
  xml: string;
}

export interface OpenXmlPackageDto {
  docx_size_bytes: number;
  files: OpenXmlFileEntryDto[];
}

export interface VirtualFile {
  path: string;
  /** UTF-8 text if editable text file (.typ, .bib, .csv, .txt, .json, .toml, .svg, .xml), null if binary */
  text: string | null;
  bytes: Uint8Array;
}

export interface StudioPreset {
  id: string;
  title: string;
  subtitle: string;
  badge: string;
  mainPath: string;
  files: Record<string, string>;
  inputs?: Record<string, string>;
}

export type StageMode = 'preview' | 'telemetry' | 'xray';

export type EnginePhase =
  | 'booting'
  | 'ready'
  | 'compiling'
  | 'fetching-package'
  | 'loading-font'
  | 'exporting'
  | 'error';

export interface EngineStatus {
  phase: EnginePhase;
  message: string;
  elapsedMs?: number;
}
