window.ptyBridge = (function () {
  let term = null;
  let fitAddon = null;
  let unlistenData = null;
  let unlistenExit = null;
  let containerId = null;
  let savedEnv = null;

  async function init(id, env) {
    const el = document.getElementById(id);
    if (!el || term) return;
    containerId = id;
    savedEnv = env;

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

    unlistenData = await __TAURI__.event.listen('pty-data', (e) => {
      term.write(e.payload);
    });

    unlistenExit = await __TAURI__.event.listen('pty-exit', () => {
      term.write('\r\n\x1b[33m[session ended — press Enter to restart]\x1b[0m\r\n');
      term.onData(async () => {
        await restart();
      });
    });

    const ro = new ResizeObserver(() => {
      try {
        fitAddon.fit();
        const { cols, rows } = term;
        __TAURI__.core.invoke('pty_resize', { cols, rows });
      } catch (_) {}
    });
    ro.observe(el);

    try {
      await __TAURI__.core.invoke('pty_create', {
        env: savedEnv || {},
        cols: term.cols,
        rows: term.rows,
      });
    } catch (e) {
      term.write('\x1b[31merror: ' + e + '\x1b[0m\r\n');
    }

    term.focus();
  }

  async function restart() {
    destroy();
    await init(containerId, savedEnv);
  }

  function destroy() {
    if (unlistenData) { unlistenData(); unlistenData = null; }
    if (unlistenExit) { unlistenExit(); unlistenExit = null; }
    if (term)         { term.dispose(); term = null; }
    fitAddon = null;
  }

  return { init, destroy, restart };
})();
