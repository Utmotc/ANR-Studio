const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])';

function focusable(panel: HTMLElement): HTMLElement[] {
  return [...panel.querySelectorAll<HTMLElement>(FOCUSABLE)].filter(
    (el) => el.offsetParent !== null || el === document.activeElement,
  );
}

/**
 * 设计文档 6.5.12：弹窗出现时把焦点移进去，Tab / Shift+Tab 在弹窗内循环，
 * 不会跑到后面的页面上。所有控件沿用项目已有的焦点环样式。
 * 返回清理函数，供 App 卸载时调用。
 */
export function installFocusTrap(): () => void {
  const seen = new WeakSet<Element>();

  const onKey = (e: KeyboardEvent) => {
    if (e.key !== 'Tab') return;
    const panels = [...document.querySelectorAll<HTMLElement>('.anim-panel')];
    if (!panels.length) return;
    const panel = panels[panels.length - 1];
    const items = focusable(panel);
    if (!items.length) return;
    const active = document.activeElement as HTMLElement | null;
    if (!panel.contains(active)) {
      e.preventDefault();
      items[0].focus();
      return;
    }
    if (e.shiftKey && active === items[0]) {
      e.preventDefault();
      items[items.length - 1].focus();
    } else if (!e.shiftKey && active === items[items.length - 1]) {
      e.preventDefault();
      items[0].focus();
    }
  };

  const observer = new MutationObserver(() => {
    const panels = [...document.querySelectorAll<HTMLElement>('.anim-panel')];
    if (!panels.length) return;
    const top = panels[panels.length - 1];
    if (seen.has(top)) return;
    seen.add(top);
    const items = focusable(top);
    const input = items.find((el) => el.tagName === 'INPUT' || el.hasAttribute('autofocus'));
    (input ?? items[0])?.focus();
  });

  document.addEventListener('keydown', onKey);
  observer.observe(document.body, { childList: true, subtree: true });

  return () => {
    document.removeEventListener('keydown', onKey);
    observer.disconnect();
  };
}
