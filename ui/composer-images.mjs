// Generated from ui-src; run npm run build:ui. Do not edit.
export const MAX_COMPOSER_IMAGES = 4;
export const MAX_COMPOSER_IMAGE_BYTES = 10 * 1024 * 1024;
export const MAX_COMPOSER_IMAGE_TOTAL_BYTES = 20 * 1024 * 1024;
const SUPPORTED_IMAGE_MIMES = new Set(['image/png', 'image/jpeg', 'image/webp', 'image/gif']);
export function detectComposerImageMime(value) {
    const bytes = value instanceof Uint8Array ? value : new Uint8Array(value || []);
    if (startsWith(bytes, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]))
        return 'image/png';
    if (startsWith(bytes, [0xff, 0xd8, 0xff]))
        return 'image/jpeg';
    if (ascii(bytes, 0, 6) === 'GIF87a' || ascii(bytes, 0, 6) === 'GIF89a')
        return 'image/gif';
    if (ascii(bytes, 0, 4) === 'RIFF' && ascii(bytes, 8, 12) === 'WEBP')
        return 'image/webp';
    return '';
}
export async function prepareComposerImage(file, id = randomImageId()) {
    if (!file || typeof file.arrayBuffer !== 'function')
        throw new Error('The selected item is not a readable image.');
    const size = Number(file.size || 0);
    if (!size)
        throw new Error('Empty images cannot be attached.');
    if (size > MAX_COMPOSER_IMAGE_BYTES)
        throw new Error('Each image must be 10 MiB or smaller.');
    const bytes = new Uint8Array(await file.arrayBuffer());
    const mime = detectComposerImageMime(bytes);
    if (!SUPPORTED_IMAGE_MIMES.has(mime))
        throw new Error('Only PNG, JPEG, WebP, and GIF images can be attached.');
    return {
        id,
        name: cleanImageName(file.name, mime),
        mime,
        size: bytes.byteLength,
        url: bytesToDataUrl(bytes, mime),
    };
}
export function composerImageInputs(images) {
    return (Array.isArray(images) ? images : [])
        .filter((image) => safeImageUrl(image?.url))
        .map((image) => ({ type: 'image', url: image.url }));
}
export function userImagesFromContent(content) {
    return (Array.isArray(content) ? content : []).flatMap((item) => {
        if (item?.type === 'image' && safeImageUrl(item.url))
            return [{ kind: 'url', source: item.url, label: 'Attached image' }];
        if (item?.type === 'localImage' && item.path)
            return [{ kind: 'local', source: String(item.path), label: imageNameFromPath(item.path) }];
        return [];
    });
}
export function openCodeImagePart(input, index = 0) {
    if (input?.type !== 'image' || !safeImageUrl(input.url))
        return null;
    const mime = dataUrlMime(input.url) || 'image/png';
    return {
        type: 'file',
        mime,
        filename: `image-${index + 1}.${extensionForMime(mime)}`,
        url: input.url,
    };
}
export function formatImageSize(bytes) {
    const size = Math.max(0, Number(bytes) || 0);
    if (size < 1024)
        return `${size} B`;
    if (size < 1024 * 1024)
        return `${Math.round(size / 1024)} KiB`;
    return `${(size / (1024 * 1024)).toFixed(1)} MiB`;
}
export function safeImageUrl(value) {
    const source = String(value || '');
    return /^data:image\/(?:png|jpeg|webp|gif);base64,[a-z0-9+/=]+$/iu.test(source);
}
function bytesToDataUrl(bytes, mime) {
    const chunks = [];
    for (let offset = 0; offset < bytes.length; offset += 0x8000) {
        chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 0x8000)));
    }
    return `data:${mime};base64,${btoa(chunks.join(''))}`;
}
function startsWith(bytes, signature) {
    return signature.every((value, index) => bytes[index] === value);
}
function ascii(bytes, start, end) {
    return String.fromCharCode(...bytes.subarray(start, end));
}
function dataUrlMime(value) {
    return String(value || '').match(/^data:(image\/(?:png|jpeg|webp|gif));base64,/iu)?.[1]?.toLowerCase() || '';
}
function cleanImageName(value, mime) {
    const name = String(value || '').replaceAll('\\', '/').split('/').pop()?.trim().slice(0, 180);
    return name || `image.${extensionForMime(mime)}`;
}
function imageNameFromPath(value) {
    return String(value || '').replaceAll('\\', '/').split('/').filter(Boolean).pop() || 'Attached image';
}
function extensionForMime(mime) {
    const extensions = { 'image/jpeg': 'jpg', 'image/webp': 'webp', 'image/gif': 'gif' };
    return extensions[mime] || 'png';
}
function randomImageId() {
    return globalThis.crypto?.randomUUID?.() || `image-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}
