import { App, PluginManifest } from 'obsidian';

const MODEL2VEC_FILES = {
  model: {
    url: 'https://huggingface.co/minishlab/potion-base-8M/resolve/main/model.safetensors',
    filename: 'model.safetensors',
  },
  tokenizer: {
    url: 'https://huggingface.co/minishlab/potion-base-8M/resolve/main/tokenizer.json',
    filename: 'tokenizer.json',
  },
  config: {
    url: 'https://huggingface.co/minishlab/potion-base-8M/resolve/main/config.json',
    filename: 'config.json',
  },
};

const MODEL_VERSION = 'potion-base-8M';

export interface DownloadProgress {
  file: string;
  percent: number;
  totalFiles: number;
  currentFile: number;
}

export class ModelDownloader {
  private app: App;
  private manifest: PluginManifest;

  constructor(app: App, manifest: PluginManifest) {
    this.app = app;
    this.manifest = manifest;
  }

  private getModelDir(): string {
    return `.obsidian/plugins/${this.manifest.id}/models/${MODEL_VERSION}`;
  }

  async downloadModel(onProgress?: (progress: DownloadProgress) => void): Promise<string> {
    const modelDir = this.getModelDir();
    await this.ensureDir(modelDir);

    const files = Object.entries(MODEL2VEC_FILES);
    let currentFile = 0;

    for (const [key, fileInfo] of files) {
      currentFile++;
      const filePath = `${modelDir}/${fileInfo.filename}`;

      await this.downloadFile(
        fileInfo.url,
        filePath,
        (percent) => {
          if (onProgress) {
            onProgress({
              file: fileInfo.filename,
              percent,
              totalFiles: files.length,
              currentFile,
            });
          }
        }
      );
    }

    return modelDir;
  }

  private async downloadFile(
    url: string,
    filePath: string,
    onProgress?: (percent: number) => void
  ): Promise<void> {
    const response = await fetch(url);

    if (!response.ok) {
      throw new Error(`Failed to download ${url}: ${response.status} ${response.statusText}`);
    }

    const contentLength = response.headers.get('Content-Length');
    const total = contentLength ? parseInt(contentLength, 10) : 0;

    if (!response.body) {
      throw new Error('Response body is null');
    }

    const reader = response.body.getReader();
    const chunks: Uint8Array[] = [];
    let receivedLength = 0;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      chunks.push(value);
      receivedLength += value.length;

      if (onProgress && total > 0) {
        onProgress(Math.round((receivedLength / total) * 100));
      }
    }

    const blob = new Blob(chunks);
    const arrayBuffer = await blob.arrayBuffer();
    await this.app.vault.adapter.writeBinary(filePath, new Uint8Array(arrayBuffer));
  }

  async deleteModel(): Promise<void> {
    const modelDir = this.getModelDir();

    if (await this.app.vault.adapter.exists(modelDir)) {
      // Delete all files in the directory
      for (const fileInfo of Object.values(MODEL2VEC_FILES)) {
        const filePath = `${modelDir}/${fileInfo.filename}`;
        if (await this.app.vault.adapter.exists(filePath)) {
          await this.app.vault.adapter.remove(filePath);
        }
      }

      // Try to remove the directory (may fail if not empty, which is fine)
      try {
        await this.app.vault.adapter.rmdir(modelDir, false);
      } catch (e) {
        // Directory might not be empty or might not exist, ignore
      }
    }
  }

  async modelExists(): Promise<boolean> {
    const modelDir = this.getModelDir();

    // Check if all required files exist
    for (const fileInfo of Object.values(MODEL2VEC_FILES)) {
      const filePath = `${modelDir}/${fileInfo.filename}`;
      if (!await this.app.vault.adapter.exists(filePath)) {
        return false;
      }
    }

    return true;
  }

  getModelPath(): string {
    return this.getModelDir();
  }

  getModelVersion(): string {
    return MODEL_VERSION;
  }

  private async ensureDir(path: string): Promise<void> {
    const parts = path.split('/');
    let currentPath = '';

    for (const part of parts) {
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      if (!await this.app.vault.adapter.exists(currentPath)) {
        await this.app.vault.adapter.mkdir(currentPath);
      }
    }
  }
}
