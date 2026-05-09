window.ptyBridge = (function () {
  let term = null;
  let fitAddon = null;
  let unlisten = null;

  async function init(containerId, env) {
    const el = document.getElementById(containerId);
    if (!el || term) return;

    term = new Terminal({
      cursorBlink: true,
      fontFamily: '"Cascadia Code", "Fira Code", "JetBrains Mono", monospace',
      fontSize: 12,
      lineHeight: 1.3,
      theme: {
        background: '#0d0d0d',
        foreground: '#c8c8c8',
        cursor:     '#4ade80',
        black:      '#1a1a1a',
        brightBlack:'#555',
        white:      '#c8c8c8',
        brightWhite:'#fff',
      },
    });

    fitAddon = new FitAddon.FitAddon();
    term.loadAddon(fitAddon);
    term.open(el);
    fitAddon.fit();

    term.onData(async (data) => {
      try { await __TAURI__.core.invoke('pty_write', { data }); } catch (_) {}
    });

    unlisten = await __TAURI__.event.listen('pty-data', (e) => {
      term.write(e.payload);
    });

    const ro = new ResizeObserver(() => {
      try {
        fitAddon.fit();
        const { cols, rows } = term;
        __TAURI__.core.invoke('pty_resize', { cols, rows });
      } catch (_) {}
    });
    ro.observe(el);

    try { await __TAURI__.core.invoke('pty_create', { env: env || {} }); } catch (e) {
      term.write('\x1b[31merror: ' + e + '\x1b[0m\r\n');
    }

    term.focus();
  }

  function destroy() {
    if (unlisten) { unlisten(); unlisten = null; }
    if (term)     { term.dispose(); term = null; }
    fitAddon = null;
  }

  return { init, destroy };
})();
