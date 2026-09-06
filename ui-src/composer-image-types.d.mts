export interface ReadableImageFile { size: number; name?: string; arrayBuffer(): Promise<ArrayBuffer> }
export interface ImageInput { type?: string; url?: string; path?: string }
export interface ComposerImage { id: string; name: string; mime: string; size: number; url: string }
