import { useEffect, useState } from 'react';

import { advanceSwimlaneTimeFrame, createSwimlaneTimeFrame } from './swimlaneClock';

export function useObservedWidth(ref: { current: HTMLElement | null }): number {
  const [width, setWidth] = useState(0);

  useEffect(() => {
    const element = ref.current;
    if (element === null || typeof ResizeObserver === 'undefined') {
      return;
    }
    const observer = new ResizeObserver((entries) => {
      const observed = entries[0];
      if (observed !== undefined) {
        setWidth(observed.contentRect.width);
      }
    });
    observer.observe(element);
    setWidth(element.getBoundingClientRect().width);
    return () => observer.disconnect();
  }, [ref]);

  return width;
}

export function useCurrentTime(running: boolean, override: number | undefined): number {
  const [frame, setFrame] = useState(() => createSwimlaneTimeFrame(override ?? Date.now()));

  useEffect(() => {
    if (override !== undefined) {
      setFrame(createSwimlaneTimeFrame(override));
      return;
    }
    if (!running) {
      return;
    }

    let interval: number | undefined;
    const refresh = () => {
      setFrame((current) => advanceSwimlaneTimeFrame(current, Date.now()));
    };
    const syncVisibility = () => {
      if (interval !== undefined) {
        window.clearInterval(interval);
        interval = undefined;
      }
      if (document.visibilityState !== 'hidden') {
        refresh();
        interval = window.setInterval(refresh, 1000);
      }
    };

    document.addEventListener('visibilitychange', syncVisibility);
    syncVisibility();
    return () => {
      document.removeEventListener('visibilitychange', syncVisibility);
      if (interval !== undefined) {
        window.clearInterval(interval);
      }
    };
  }, [override, running]);

  return override ?? frame.nowMs;
}
