import type {
  ConversionReport,
  EnginePhase,
  FontEmbedMode,
  FontFamilyDto,
  OpenXmlPackageDto,
  PreviewResponseDto,
  VirtualFile,
} from './types';

interface PendingCall {
  resolve: (value: any) => void;
  reject: (reason?: any) => void;
}

export interface StudioWorkerCallbacks {
  onBooted?: (data: { fonts: FontFamilyDto[]; packages: string[] }) => void;
  onStatus?: (phase: EnginePhase, message: string) => void;
  onFontsUpdated?: (fonts: FontFamilyDto[], autoRecompile: boolean) => void;
  onPackagesUpdated?: (packages: string[]) => void;
}

export class TypstWordWorkerClient {
  private worker: Worker;
  private nextId = 1;
  private pending = new Map<number, PendingCall>();
  private callbacks: StudioWorkerCallbacks;

  constructor(callbacks: StudioWorkerCallbacks = {}) {
    this.callbacks = callbacks;
    this.worker = new Worker(new URL('./compiler.worker.ts', import.meta.url), {
      type: 'module',
    });

    this.worker.onmessage = (event: MessageEvent) => {
      const data = event.data;
      if (!data) return;

      if (typeof data.type === 'string') {
        switch (data.type) {
          case 'booted':
            this.callbacks.onBooted?.({
              fonts: data.fonts ?? [],
              packages: data.packages ?? [],
            });
            break;
          case 'status':
            this.callbacks.onStatus?.(data.phase, data.message);
            break;
          case 'fonts-updated':
            this.callbacks.onFontsUpdated?.(data.fonts ?? [], Boolean(data.autoRecompile));
            break;
          case 'packages-updated':
            this.callbacks.onPackagesUpdated?.(data.packages ?? []);
            break;
        }
        return;
      }

      if (typeof data.id === 'number') {
        const entry = this.pending.get(data.id);
        if (!entry) return;
        this.pending.delete(data.id);
        if (data.ok) {
          entry.resolve(data.result);
        } else {
          entry.reject(new Error(data.error || 'Worker call failed'));
        }
      }
    };

    this.worker.onerror = (err) => {
      this.callbacks.onStatus?.('error', err.message || 'Web Worker 运行异常');
    };
  }

  private call<T>(action: string, payload?: unknown, transfer?: Transferable[]): Promise<T> {
    const id = this.nextId++;
    return new Promise<T>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      if (transfer && transfer.length > 0) {
        this.worker.postMessage({ id, action, payload }, transfer);
      } else {
        this.worker.postMessage({ id, action, payload });
      }
    });
  }

  boot(): Promise<{ fonts: FontFamilyDto[]; packages: string[] }> {
    return this.call('boot');
  }

  private static toPlainWorkspace(params: {
    mainPath: string;
    files: VirtualFile[];
    inputs: Record<string, string>;
  }) {
    const plainInputs: Record<string, string> = {};
    for (const [k, v] of Object.entries(params.inputs || {})) {
      plainInputs[String(k)] = String(v);
    }
    return {
      mainPath: String(params.mainPath),
      files: Array.from(params.files, (f) => ({
        path: String(f.path),
        bytes: new Uint8Array(f.bytes),
        text: f.text === null ? null : String(f.text),
      })),
      inputs: plainInputs,
    };
  }

  syncAndPreview(params: {
    mainPath: string;
    files: VirtualFile[];
    inputs: Record<string, string>;
  }): Promise<{
    preview: PreviewResponseDto;
    elapsedMs: number;
    fonts: FontFamilyDto[];
    packages: string[];
  }> {
    return this.call('sync-and-preview', TypstWordWorkerClient.toPlainWorkspace(params));
  }

  compileOnlyPreview(): Promise<{
    preview: PreviewResponseDto;
    elapsedMs: number;
    fonts: FontFamilyDto[];
    packages: string[];
  }> {
    return this.call('compile-only-preview');
  }

  async exportDocx(
    workspace?: {
      mainPath: string;
      files: VirtualFile[];
      inputs: Record<string, string>;
    },
    embedMode: FontEmbedMode = 'subset',
  ): Promise<{
    docxBytes: Uint8Array;
    report: ConversionReport;
    elapsedMs: number;
    fonts: FontFamilyDto[];
    packages: string[];
  }> {
    const res = await this.call<{
      docxBuffer: ArrayBuffer;
      report: ConversionReport;
      elapsedMs: number;
      fonts?: FontFamilyDto[];
      packages?: string[];
    }>('export-docx', {
      workspace: workspace
        ? TypstWordWorkerClient.toPlainWorkspace(workspace)
        : undefined,
      embedMode,
    });
    return {
      docxBytes: new Uint8Array(res.docxBuffer),
      report: res.report,
      elapsedMs: res.elapsedMs,
      fonts: res.fonts ?? [],
      packages: res.packages ?? [],
    };
  }

  inspectOpenXml(
    workspace?: {
      mainPath: string;
      files: VirtualFile[];
      inputs: Record<string, string>;
    },
    embedMode: FontEmbedMode = 'subset',
  ): Promise<OpenXmlPackageDto> {
    return this.call('inspect-openxml', {
      workspace: workspace
        ? TypstWordWorkerClient.toPlainWorkspace(workspace)
        : undefined,
      embedMode,
    });
  }

  inspectIr(workspace?: {
    mainPath: string;
    files: VirtualFile[];
    inputs: Record<string, string>;
  }): Promise<{ irJson: string }> {
    return this.call('inspect-ir', {
      workspace: workspace
        ? TypstWordWorkerClient.toPlainWorkspace(workspace)
        : undefined,
    });
  }

  addFonts(
    fonts: Array<{ name: string; buffer: ArrayBuffer; isSystemInstalled: boolean }>,
  ): Promise<{ addedFaces: number; fonts: FontFamilyDto[] }> {
    const transfers = fonts.map((f) => f.buffer);
    return this.call('add-fonts', { fonts }, transfers);
  }

  loadCloudCjk(): Promise<{ addedFaces: number; fonts: FontFamilyDto[] }> {
    return this.call('load-cloud-cjk');
  }

  destroy(): void {
    this.worker.terminate();
    for (const [, p] of this.pending) {
      p.reject(new Error('Worker terminated'));
    }
    this.pending.clear();
  }
}
