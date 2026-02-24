import { useEffect, useState, type RefObject } from 'react';
import { DiffSide } from '@/types/diff';

interface HighlightRange {
  side: DiffSide;
  startLine: number;
  endLine: number;
}

interface RangeHighlightOverlayProps {
  containerRef: RefObject<HTMLDivElement | null>;
  range: HighlightRange;
}

interface HighlightRect {
  top: number;
  height: number;
}

/**
 * Renders translucent highlight rectangles over diff lines in a range.
 * Queries the @pierre/diffs shadow DOM to find line element positions.
 */
export function RangeHighlightOverlay({
  containerRef,
  range,
}: RangeHighlightOverlayProps) {
  const [rects, setRects] = useState<HighlightRect[]>([]);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) {
      setRects([]);
      return;
    }

    const diffsEl = container.querySelector('diffs-container');
    const shadow = diffsEl?.shadowRoot;
    if (!shadow) {
      setRects([]);
      return;
    }

    const containerRect = container.getBoundingClientRect();

    // Find all line rows in the shadow DOM and check if any of their
    // number columns fall within our range
    const lineRows = shadow.querySelectorAll('[data-line-type]');
    const matchedRects: DOMRect[] = [];
    const seenTops = new Set<number>();

    lineRows.forEach((row) => {
      const numberCols = row.querySelectorAll('[data-column-number]');
      let matched = false;

      numberCols.forEach((col) => {
        const text = col.textContent?.trim();
        if (!text) return;
        const lineNum = parseInt(text, 10);
        if (isNaN(lineNum)) return;
        if (lineNum >= range.startLine && lineNum <= range.endLine) {
          matched = true;
        }
      });

      if (matched) {
        const rect = row.getBoundingClientRect();
        // Deduplicate rows at the same vertical position
        const roundedTop = Math.round(rect.top);
        if (!seenTops.has(roundedTop)) {
          seenTops.add(roundedTop);
          matchedRects.push(rect);
        }
      }
    });

    if (matchedRects.length === 0) {
      setRects([]);
      return;
    }

    // Merge rows into contiguous highlight rectangles
    const sorted = matchedRects.sort((a, b) => a.top - b.top);
    const merged: HighlightRect[] = [];
    let current = {
      top: sorted[0].top - containerRect.top,
      bottom: sorted[0].bottom - containerRect.top,
    };

    for (let i = 1; i < sorted.length; i++) {
      const rowTop = sorted[i].top - containerRect.top;
      const rowBottom = sorted[i].bottom - containerRect.top;
      if (rowTop <= current.bottom + 2) {
        current.bottom = Math.max(current.bottom, rowBottom);
      } else {
        merged.push({ top: current.top, height: current.bottom - current.top });
        current = { top: rowTop, bottom: rowBottom };
      }
    }
    merged.push({ top: current.top, height: current.bottom - current.top });

    setRects(merged);
  }, [containerRef, range.startLine, range.endLine, range.side]);

  if (rects.length === 0) return null;

  return (
    <>
      {rects.map((rect, i) => (
        <div
          key={i}
          className="diff-range-highlight-bar"
          style={{
            position: 'absolute',
            top: rect.top,
            left: 0,
            right: 0,
            height: rect.height,
            pointerEvents: 'none',
            zIndex: 3,
          }}
        />
      ))}
    </>
  );
}
