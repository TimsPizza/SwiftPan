export async function quickHash(_blob: Blob): Promise<string> {
  // lightweight placeholder hash: size + lastModified if File
  const size = _blob.size;
  const lm = typeof File !== 'undefined' && _blob instanceof File ? _blob.lastModified : 0;
  return `h-${size}-${lm}`;
}
