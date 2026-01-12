import { App, Notice } from 'obsidian';
import { Model2VecEncoder } from '../wasm-pkg/elysium_wasm';

/**
 * ModelLoader service for loading Model2Vec into WASM
 *
 * This service handles loading the Model2Vec model files from disk
 * and initializing the WASM encoder.
 */
export class ModelLoader {
  private app: App;
  private encoder: Model2VecEncoder | null = null;
  private modelPath: string | null = null;

  constructor(app: App) {
    this.app = app;
  }

  /**
   * Load Model2Vec model from the specified path
   *
   * @param modelPath - Path to the model directory (e.g., ".obsidian/plugins/elysium/models/potion-base-8M")
   */
  async loadModel(modelPath: string): Promise<void> {
    const adapter = this.app.vault.adapter;

    // Check if model files exist
    const modelFile = `${modelPath}/model.safetensors`;
    const tokenizerFile = `${modelPath}/tokenizer.json`;
    const configFile = `${modelPath}/config.json`;

    const modelExists = await adapter.exists(modelFile);
    const tokenizerExists = await adapter.exists(tokenizerFile);
    const configExists = await adapter.exists(configFile);

    if (!modelExists || !tokenizerExists || !configExists) {
      throw new Error(
        `Model files not found at ${modelPath}. ` +
        `Missing: ${[
          !modelExists && 'model.safetensors',
          !tokenizerExists && 'tokenizer.json',
          !configExists && 'config.json',
        ].filter(Boolean).join(', ')}`
      );
    }

    console.log('[Elysium] Loading Model2Vec from:', modelPath);

    // Read files as binary
    const [modelBuffer, tokenizerBuffer, configBuffer] = await Promise.all([
      adapter.readBinary(modelFile),
      adapter.readBinary(tokenizerFile),
      adapter.readBinary(configFile),
    ]);

    console.log('[Elysium] Model file sizes:', {
      model: modelBuffer.byteLength,
      tokenizer: tokenizerBuffer.byteLength,
      config: configBuffer.byteLength,
    });

    // Create and load encoder
    this.encoder = new Model2VecEncoder();

    try {
      this.encoder.load(
        new Uint8Array(modelBuffer),
        new Uint8Array(tokenizerBuffer),
        new Uint8Array(configBuffer),
      );
    } catch (e) {
      this.encoder = null;
      throw new Error(`Failed to load model: ${e}`);
    }

    this.modelPath = modelPath;
    console.log('[Elysium] Model2Vec loaded successfully:', {
      dim: this.encoder.dim(),
      vocabSize: this.encoder.vocab_size(),
    });
  }

  /**
   * Encode text to 256D embedding vector
   */
  encode(text: string): Float32Array {
    if (!this.encoder || !this.encoder.is_loaded()) {
      throw new Error('Model not loaded. Call loadModel() first.');
    }
    return new Float32Array(this.encoder.encode(text));
  }

  /**
   * Check if model is loaded
   */
  isLoaded(): boolean {
    return this.encoder?.is_loaded() ?? false;
  }

  /**
   * Get embedding dimension (256 for potion-base-8M)
   */
  getDim(): number {
    return this.encoder?.dim() ?? 256;
  }

  /**
   * Get vocabulary size
   */
  getVocabSize(): number {
    return this.encoder?.vocab_size() ?? 0;
  }

  /**
   * Get current model path
   */
  getModelPath(): string | null {
    return this.modelPath;
  }

  /**
   * Unload the model to free memory
   */
  unload(): void {
    this.encoder = null;
    this.modelPath = null;
    console.log('[Elysium] Model2Vec unloaded');
  }
}
