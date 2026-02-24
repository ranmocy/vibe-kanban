import type { FileDiffMetadata } from '@pierre/diffs';

const MAX_CACHE_SIZE = 100;
const cache = new Map<string, FileDiffMetadata>();

/**
 * Simple non-cryptographic hash (djb2 variant) for generating cache keys.
 * Only needs to be collision-resistant enough for an LRU cache, not secure.
 */
function simpleHash(str: string): number {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = ((hash << 5) - hash + str.charCodeAt(i)) | 0;
  }
  return hash;
}

function cacheKey(
  oldContent: string,
  newContent: string,
  ignoreWhitespace: boolean
): string {
  return `${oldContent.length}:${newContent.length}:${ignoreWhitespace}:${simpleHash(oldContent)}:${simpleHash(newContent)}`;
}

export function getCachedDiffMetadata(
  oldContent: string,
  newContent: string,
  ignoreWhitespace: boolean
): FileDiffMetadata | undefined {
  const key = cacheKey(oldContent, newContent, ignoreWhitespace);
  const result = cache.get(key);
  if (result) {
    // Move to end for LRU ordering (Map preserves insertion order)
    cache.delete(key);
    cache.set(key, result);
  }
  return result;
}

export function setCachedDiffMetadata(
  oldContent: string,
  newContent: string,
  ignoreWhitespace: boolean,
  metadata: FileDiffMetadata
): void {
  const key = cacheKey(oldContent, newContent, ignoreWhitespace);
  cache.set(key, metadata);
  // Evict oldest entry if over limit
  if (cache.size > MAX_CACHE_SIZE) {
    const firstKey = cache.keys().next().value;
    if (firstKey !== undefined) cache.delete(firstKey);
  }
}
