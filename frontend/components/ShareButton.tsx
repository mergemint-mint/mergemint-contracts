'use client';

import { useEffect, useMemo, useRef, useState } from 'react';

interface ShareButtonProps {
  bountyId: string;
  bountyTitle?: string;
  url?: string;
}

const QR_SIZE = 220;
const QUIET_ZONE = 4;

// Minimal QR encoder (byte mode, error correction level L, versions 1-10).
// Kept dependency-free so the QR is generated fully client-side.
const EC_LEVEL_L = 0;
const EC_CODEWORDS_PER_BLOCK_L = [
  7, 10, 15, 20, 26, 18, 20, 24, 30, 18,
];
const NUM_EC_BLOCKS_L = [1, 1, 1, 1, 1, 2, 2, 2, 2, 4];
const ALIGNMENT_POSITIONS = [
  [],
  [],
  [6, 18],
  [6, 22],
  [6, 26],
  [6, 30],
  [6, 34],
  [6, 22, 38],
  [6, 24, 42],
  [6, 26, 46],
];

function getDataCapacityBytes(version: number): number {
  const totalCodewords = getTotalCodewords(version);
  const ecCodewords = EC_CODEWORDS_PER_BLOCK_L[version - 1] * NUM_EC_BLOCKS_L[version - 1];
  const dataCodewords = totalCodewords - ecCodewords;
  // 4 bits mode + 8/16 bits length indicator
  const lengthBits = version <= 9 ? 8 : 16;
  return Math.floor((dataCodewords * 8 - 4 - lengthBits) / 8);
}

function getTotalCodewords(version: number): number {
  let result = (16 * version + 128) * version + 64;
  if (version >= 2) {
    const numAlign = Math.floor(version / 7) + 2;
    result -= (25 * numAlign - 10) * numAlign - 55;
    if (version >= 7) result -= 36;
  }
  return Math.floor(result / 8);
}

function utf8Bytes(text: string): number[] {
  const encoded = new TextEncoder().encode(text);
  return Array.from(encoded);
}

function chooseVersion(byteLength: number): number {
  for (let v = 1; v <= 10; v++) {
    if (byteLength <= getDataCapacityBytes(v)) return v;
  }
  return 10;
}

function buildDataCodewords(bytes: number[], version: number): number[] {
  const bits: number[] = [];
  const pushBits = (value: number, length: number) => {
    for (let i = length - 1; i >= 0; i--) bits.push((value >>> i) & 1);
  };
  pushBits(0b0100, 4);
  pushBits(bytes.length, version <= 9 ? 8 : 16);
  for (const b of bytes) pushBits(b, 8);

  const totalCodewords = getTotalCodewords(version);
  const ecCodewords = EC_CODEWORDS_PER_BLOCK_L[version - 1] * NUM_EC_BLOCKS_L[version - 1];
  const dataCodewords = totalCodewords - ecCodewords;
  const capacityBits = dataCodewords * 8;
  pushBits(0, Math.min(4, capacityBits - bits.length));
  while (bits.length % 8 !== 0) bits.push(0);

  const codewords: number[] = [];
  for (let i = 0; i < bits.length; i += 8) {
    let byte = 0;
    for (let j = 0; j < 8; j++) byte = (byte << 1) | bits[i + j];
    codewords.push(byte);
  }
  let pad = 0xec;
  while (codewords.length < dataCodewords) {
    codewords.push(pad);
    pad = pad === 0xec ? 0x11 : 0xec;
  }
  return codewords;
}

function gfMultiply(x: number, y: number): number {
  let z = 0;
  for (let i = 7; i >= 0; i--) {
    z = (z << 1) ^ ((z >>> 7) * 0x11d);
    z ^= ((y >>> i) & 1) * x;
  }
  return z & 0xff;
}

function reedSolomonDivisor(degree: number): number[] {
  const result: number[] = [];
  for (let i = 0; i < degree - 1; i++) result.push(0);
  result.push(1);
  let root = 1;
  for (let i = 0; i < degree; i++) {
    for (let j = 0; j < result.length; j++) {
      result[j] = gfMultiply(result[j], root);
      if (j + 1 < result.length) result[j] ^= result[j + 1];
    }
    root = gfMultiply(root, 0x02);
  }
  return result;
}

function reedSolomonRemainder(data: number[], divisor: number[]): number[] {
  const result = divisor.map(() => 0);
  for (const b of data) {
    const factor = b ^ (result.shift() as number);
    result.push(0);
    for (let i = 0; i < divisor.length; i++) {
      result[i] ^= gfMultiply(divisor[i], factor);
    }
  }
  return result;
}

function addEccAndInterleave(data: number[], version: number): number[] {
  const numBlocks = NUM_EC_BLOCKS_L[version - 1];
  const blockEccLen = EC_CODEWORDS_PER_BLOCK_L[version - 1];
  const rawCodewords = getTotalCodewords(version);
  const numShortBlocks = numBlocks - (rawCodewords % numBlocks);
  const shortBlockLen = Math.floor(rawCodewords / numBlocks);

  const blocks: number[][] = [];
  const divisor = reedSolomonDivisor(blockEccLen);
  for (let i = 0, k = 0; i < numBlocks; i++) {
    const dat = data.slice(k, k + shortBlockLen - blockEccLen + (i < numShortBlocks ? 0 : 1));
    k += dat.length;
    const ecc = reedSolomonRemainder(dat, divisor);
    if (i < numShortBlocks) dat.push(0);
    blocks.push(dat.concat(ecc));
  }

  const result: number[] = [];
  for (let i = 0; i < blocks[0].length; i++) {
    for (let j = 0; j < blocks.length; j++) {
      if (i !== shortBlockLen - blockEccLen || j >= numShortBlocks) {
        result.push(blocks[j][i]);
      }
    }
  }
  return result;
}

function buildMatrix(version: number, codewords: number[]): boolean[][] {
  const size = version * 4 + 17;
  const modules: boolean[][] = Array.from({ length: size }, () => Array(size).fill(false));
  const isFunction: boolean[][] = Array.from({ length: size }, () => Array(size).fill(false));

  const setFunction = (x: number, y: number, dark: boolean) => {
    modules[y][x] = dark;
    isFunction[y][x] = true;
  };

  // Finder patterns
  const drawFinder = (cx: number, cy: number) => {
    for (let dy = -4; dy <= 4; dy++) {
      for (let dx = -4; dx <= 4; dx++) {
        const x = cx + dx;
        const y = cy + dy;
        if (x < 0 || x >= size || y < 0 || y >= size) continue;
        const dist = Math.max(Math.abs(dx), Math.abs(dy));
        setFunction(x, y, dist !== 2 && dist !== 4);
      }
    }
  };
  drawFinder(3, 3);
  drawFinder(size - 4, 3);
  drawFinder(3, size - 4);

  // Timing patterns
  for (let i = 0; i < size; i++) {
    if (!isFunction[6][i]) setFunction(i, 6, i % 2 === 0);
    if (!isFunction[i][6]) setFunction(6, i, i % 2 === 0);
  }

  // Alignment patterns
  const positions = ALIGNMENT_POSITIONS[version - 1];
  for (const ay of positions) {
    for (const ax of positions) {
      if (isFunction[ay][ax]) continue;
      for (let dy = -2; dy <= 2; dy++) {
        for (let dx = -2; dx <= 2; dx++) {
          const dist = Math.max(Math.abs(dx), Math.abs(dy));
          setFunction(ax + dx, ay + dy, dist !== 1);
        }
      }
    }
  }

  // Reserve format areas
  for (let i = 0; i < 9; i++) {
    if (!isFunction[8][i]) setFunction(i, 8, false);
    if (!isFunction[i][8]) setFunction(8, i, false);
  }
  for (let i = 0; i < 8; i++) {
    if (!isFunction[8][size - 1 - i]) setFunction(size - 1 - i, 8, false);
    if (!isFunction[size - 1 - i][8]) setFunction(8, size - 1 - i, false);
  }
  setFunction(8, size - 8, true);

  // Place data bits
  let bitIndex = 0;
  for (let right = size - 1; right >= 1; right -= 2) {
    if (right === 6) right = 5;
    for (let vert = 0; vert < size; vert++) {
      for (let j = 0; j < 2; j++) {
        const x = right - j;
        const upward = ((right + 1) & 2) === 0;
        const y = upward ? size - 1 - vert : vert;
        if (!isFunction[y][x] && bitIndex < codewords.length * 8) {
          modules[y][x] = ((codewords[bitIndex >>> 3] >>> (7 - (bitIndex & 7))) & 1) !== 0;
          bitIndex++;
        }
      }
    }
  }

  // Apply mask 0 and format info
  const mask = 0;
  const applyMask = (x: number, y: number) => (x + y) % 2 === 0;
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      if (!isFunction[y][x] && applyMask(x, y)) modules[y][x] = !modules[y][x];
    }
  }

  const formatBits = (EC_LEVEL_L << 3) | mask;
  let rem = formatBits;
  for (let i = 0; i < 10; i++) rem = (rem << 1) ^ ((rem >>> 9) * 0x537);
  const bits = ((formatBits << 10) | rem) ^ 0x5412;

  for (let i = 0; i <= 5; i++) setFunction(8, i, ((bits >>> i) & 1) !== 0);
  setFunction(8, 7, ((bits >>> 6) & 1) !== 0);
  setFunction(8, 8, ((bits >>> 7) & 1) !== 0);
  setFunction(7, 8, ((bits >>> 8) & 1) !== 0);
  for (let i = 9; i < 15; i++) setFunction(14 - i, 8, ((bits >>> i) & 1) !== 0);

  for (let i = 0; i < 8; i++) setFunction(size - 1 - i, 8, ((bits >>> i) & 1) !== 0);
  for (let i = 8; i < 15; i++) setFunction(8, size - 15 + i, ((bits >>> i) & 1) !== 0);
  setFunction(8, size - 8, true);

  return modules;
}

function generateQrMatrix(text: string): boolean[][] {
  const bytes = utf8Bytes(text);
  const version = chooseVersion(bytes.length);
  const data = buildDataCodewords(bytes, version);
  const codewords = addEccAndInterleave(data, version);
  return buildMatrix(version, codewords);
}

export default function ShareButton({ bountyId, bountyTitle, url }: ShareButtonProps) {
  const [open, setOpen] = useState(false);
  const [copied, setCopied] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  const shareUrl = useMemo(() => {
    if (url) return url;
    if (typeof window !== 'undefined') {
      return `${window.location.origin}/bounties/${bountyId}`;
    }
    return `/bounties/${bountyId}`;
  }, [url, bountyId]);

  const qrMatrix = useMemo(() => {
    try {
      return generateQrMatrix(shareUrl);
    } catch {
      return null;
    }
  }, [shareUrl]);

  useEffect(() => {
    if (!open) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [open]);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(shareUrl);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard may be unavailable; ignore silently.
    }
  };

  const handleDownload = () => {
    if (!qrMatrix) return;
    const scale = 8;
    const total = (qrMatrix.length + QUIET_ZONE * 2) * scale;
    const canvas = document.createElement('canvas');
    canvas.width = total;
    canvas.height = total;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const isDark = document.documentElement.classList.contains('dark');
    ctx.fillStyle = isDark ? '#0f172a' : '#ffffff';
    ctx.fillRect(0, 0, total, total);
    ctx.fillStyle = isDark ? '#f8fafc' : '#0f172a';

    for (let y = 0; y < qrMatrix.length; y++) {
      for (let x = 0; x < qrMatrix.length; x++) {
        if (qrMatrix[y][x]) {
          ctx.fillRect(
            (x + QUIET_ZONE) * scale,
            (y + QUIET_ZONE) * scale,
            scale,
            scale
          );
        }
      }
    }

    const link = document.createElement('a');
    link.download = `bounty-${bountyId}-qr.png`;
    link.href = canvas.toDataURL('image/png');
    link.click();
  };

  return (
    <div className="relative inline-block" ref={menuRef}>
      <button
        type="button"
        onClick={() => setOpen((prev) => !prev)}
        aria-haspopup="menu"
        aria-expanded={open}
        className="inline-flex items-center gap-2 rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-200 dark:hover:bg-gray-700"
      >
        <svg
          xmlns="http://www.w3.org/2000/svg"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
          className="h-4 w-4"
          aria-hidden="true"
        >
          <circle cx="18" cy="5" r="3" />
          <circle cx="6" cy="12" r="3" />
          <circle cx="18" cy="19" r="3" />
          <line x1="8.59" y1="13.51" x2="15.42" y2="17.49" />
          <line x1="15.41" y1="6.51" x2="8.59" y2="10.49" />
        </svg>
        Share
      </button>

      {open && (
        <div
          role="menu"
          className="absolute right-0 z-20 mt-2 w-72 rounded-xl border border-gray-200 bg-white p-4 shadow-lg dark:border-gray-700 dark:bg-gray-800"
        >
          <p className="mb-3 text-sm font-semibold text-gray-900 dark:text-gray-100">
            {bountyTitle ? `Share “${bountyTitle}”` : 'Share this bounty'}
          </p>

          <div className="flex justify-center rounded-lg bg-white p-3 dark:bg-gray-900">
            {qrMatrix ? (
              <svg
                viewBox={`0 0 ${qrMatrix.length + QUIET_ZONE * 2} ${qrMatrix.length + QUIET_ZONE * 2}`}
                width={QR_SIZE}
                height={QR_SIZE}
                role="img"
                aria-label="QR code for bounty link"
                className="h-auto w-full max-w-[220px]"
              >
                <rect
                  width={qrMatrix.length + QUIET_ZONE * 2}
                  height={qrMatrix.length + QUIET_ZONE * 2}
                  className="fill-white dark:fill-gray-900"
                />
                {qrMatrix.map((row, y) =>
                  row.map((cell, x) =>
                    cell ? (
                      <rect
                        key={`${x}-${y}`}
                        x={x + QUIET_ZONE}
                        y={y + QUIET_ZONE}
                        width={1}
                        height={1}
                        className="fill-gray-900 dark:fill-gray-100"
                      />
                    ) : null
                  )
                )}
              </svg>
            ) : (
              <p className="py-8 text-center text-sm text-gray-500 dark:text-gray-400">
                Unable to generate QR code
              </p>
            )}
          </div>

          <div className="mt-4 flex flex-col gap-2">
            <button
              type="button"
              onClick={handleDownload}
              disabled={!qrMatrix}
              className="w-full rounded-lg bg-indigo-600 px-3 py-2 text-sm font-medium text-white transition-colors hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50"
            >
              Download PNG
            </button>
            <button
              type="button"
              onClick={handleCopy}
              className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-50 dark:border-gray-600 dark:text-gray-200 dark:hover:bg-gray-700"
            >
              {copied ? 'Copied!' : 'Copy link'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
