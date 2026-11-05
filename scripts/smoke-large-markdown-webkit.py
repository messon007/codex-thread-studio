import json
import sys
import gi

gi.require_version('Gtk', '3.0')
gi.require_version('WebKit2', '4.1')
from gi.repository import Gtk, WebKit2, GLib

manager = WebKit2.UserContentManager()
manager.register_script_message_handler('result')
view = WebKit2.WebView(web_context=WebKit2.WebContext.new_ephemeral(), user_content_manager=manager)
window = Gtk.Window()
window.set_default_size(900, 900)
window.add(view)
result = None


def received(manager, message):
    global result
    result = json.loads(message.get_js_value().to_string())
    Gtk.main_quit()


def loaded(view, event):
    if event != WebKit2.LoadEvent.FINISHED:
        return
    view.run_javascript(r"""
    (async () => {
      const wait = async predicate => {
        const deadline = performance.now() + 30000;
        while (!predicate()) {
          if (performance.now() > deadline) throw Error('Reader operation timed out: ' + document.querySelector('#status').textContent);
          await new Promise(resolve => setTimeout(resolve, 30));
        }
      };
      try {
        await wait(() => window.readerExperiment?.metrics.firstContentMs != null);
        const reader = window.readerExperiment;
        const { createReaderClient } = await import('/reader-assets/host-client.mjs');
        const client = createReaderClient({ src: location.href, contentWindow: window });
        await client.ready();
        await client.request('settings', { toolbar: 'host' });
        if (__TABLE_CHECK__) {
          const owner = await client.request('state');
          const info = await client.request('tableInfo', { table: 0 });
          if (info.rows !== 123) throw Error('Table record count changed');
          const total = await client.request('sql', { table: 0, sql: 'SELECT COUNT(*),SUM(CAST(c2 AS INTEGER)),MIN(c3) FROM t' });
          if (JSON.stringify(total.rows[0]) !== JSON.stringify(['123','7503','001'])) throw Error('Whole-input SQL changed values');
          await client.request('sqlClose', { id: total.id });
          const chart = await client.request('tableChart', { source: { kind: 'table', table: 0 }, x: 0, y: 1 });
          if (chart.points.length !== 123 || chart.points[122].value !== 122) throw Error('Chart used a partial dataset');
          await client.request('navigate', { location: { kind: 'cell', table: 0, row: 122, column: 2 } });
          const selected = await client.request('selection');
          if (selected?.quote !== '001' || selected.precision !== 'exact' || selected.location.row !== 122 || selected.location.column !== 2) throw Error('Original cell reference lost');
          if (owner.format === 'xlsx') {
            if (owner.capabilities.source || owner.capabilities.edit || owner.document.tables !== 2) throw Error('Workbook capabilities changed');
            const second = await client.request('sql', { table: 1, sql: 'SELECT c1 FROM t' });
            if (second.rows[0][0] !== 'Second-sheet sentinel') throw Error('Worksheet dataset crossed');
            await client.request('sqlClose', { id: second.id });
          } else {
            await client.request('setView', { view: 'source' });
            const source = await client.request('sourceWindow', { line: 1 });
            if (!source.result.text.includes('row-122')) throw Error('Original source unavailable');
          }
          await client.dispose({ force: true });
          window.webkit.messageHandlers.result.postMessage(JSON.stringify({ format: owner.format, rows: 123, wholeInputQuery: true, typedCells: true, cleanup: client.active === 0 }));
          return;
        }

        if (reader.index.tables.length) {
          await reader.jump(reader.index.tables[0].firstPage);
          await wait(() => document.querySelector('.column-choices input'));
          document.querySelector('.column-picker').open = true;
          const input = document.querySelector('.column-choices input');
          const style = getComputedStyle(input);
          const mark = getComputedStyle(input, '::after');
          if (input.getBoundingClientRect().width !== 16 || style.padding !== '0px' || mark.left !== '7px' || mark.top !== '7px' || !mark.clipPath.startsWith('polygon(')) throw Error('Shared checkbox not centered: ' + JSON.stringify({ width: input.getBoundingClientRect().width, padding: style.padding, left: mark.left, top: mark.top, clip: mark.clipPath }));
          document.querySelector('.column-picker').open = false;
        }
        for (const diagram of reader.index.diagrams.slice(0, 3)) {
          await reader.jump(diagram.page);
          await wait(() => {
            const figure = document.querySelector(`figure[data-diagram-id="${diagram.id}"]`);
            if (figure?.dataset.diagramState === 'error') throw Error(figure.textContent);
            return figure?.dataset.diagramState === 'rendered';
          });
        }
        await reader.source.show(Math.max(1, Math.floor(reader.index.sourceLines / 2)));
        if (!reader.source.page?.text || !reader.source.visible) throw Error('Paged Source did not load');
        if (__EDIT_CHECK__) {
          await client.request('setView', { view: 'edit' });
          document.querySelector('#edit-line').value = '1';
          document.querySelector('#edit-go').dispatchEvent(new Event('submit', { cancelable: true }));
          await wait(() => readerExperiment.editor.data?.window.start === 0 && !readerExperiment.editor.busy);
          const code = readerExperiment.editor.code;
          code.view.dispatch({ changes: { from: 0, insert: 'Native Studio edit\n' } });
          if (!readerExperiment.editor.dirty) throw Error('Native edit did not become dirty');
          await client.request('setView', { view: 'source' });
          if (document.querySelector('#source-text').textContent.startsWith('Native Studio edit')) throw Error('View switching saved unexpectedly');
          await client.request('setView', { view: 'edit' });
          if (!readerExperiment.editor.code.text.startsWith('Native Studio edit')) throw Error('Native Edit lost its page');
          await client.request('save');
          if (readerExperiment.editor.dirty) throw Error('Native Save did not clear dirty');
          await client.request('setView', { view: 'source' });
          if (!document.querySelector('#source-text').textContent.startsWith('Native Studio edit')) throw Error('Saved source did not refresh');
        }
        window.webkit.messageHandlers.result.postMessage(JSON.stringify({sourceBytes: reader.index.sourceBytes, diagrams: reader.index.diagrams.length, mode: reader.diagramMetrics.mode, sourceReady: true, editing: __EDIT_CHECK__, mounted: document.querySelectorAll('#space *').length}));
      } catch (error) {
        window.webkit.messageHandlers.result.postMessage(JSON.stringify({error: String(error)}));
      }
    })()
    """.replace('__EDIT_CHECK__', 'true' if '--edit' in sys.argv else 'false').replace('__TABLE_CHECK__', 'true' if '--tables' in sys.argv else 'false'), None, None, None)


manager.connect('script-message-received::result', received)
view.connect('load-changed', loaded)
window.show_all()
view.load_uri(sys.argv[1])
GLib.timeout_add_seconds(100, lambda: (Gtk.main_quit(), False)[1])
Gtk.main()
window.destroy()
if result is None or 'error' in result:
    raise RuntimeError(result or 'Native WebKit smoke timed out')
print(json.dumps(result))
