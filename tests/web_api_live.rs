//! Live webpage + Web API smoke test (rv8-v8 software-render path).

use rv8::engine::BrowserEngine;
use rv8::servo_embed::ServoConfig;

#[cfg(feature = "rv8-v8")]
#[tokio::test]
async fn live_page_dom_and_apis_smoke() {
    let mut engine = BrowserEngine::new(ServoConfig::default())
        .await
        .expect("engine");

    engine
        .navigate("https://example.com")
        .await
        .expect("navigate example.com");

    assert!(!engine.title().is_empty() || engine.current_url().contains("example.com"));

    let globals = engine
        .evaluate_script(
            "JSON.stringify({
               hasHTMLElement: typeof HTMLElement === 'function',
               hasMutationObserver: typeof MutationObserver === 'function',
               hasIntersectionObserver: typeof IntersectionObserver === 'function',
               hasWebSocket: typeof WebSocket === 'function',
               hasWorker: typeof Worker === 'function',
               hasIndexedDB: typeof indexedDB === 'object'
             })",
        )
        .await
        .expect("globals probe");
    assert!(globals.contains("\"hasHTMLElement\":true"));
    assert!(globals.contains("\"hasMutationObserver\":true"));
    assert!(globals.contains("\"hasIntersectionObserver\":true"));
    assert!(globals.contains("\"hasWebSocket\":true"));
    assert!(globals.contains("\"hasWorker\":true"));
    assert!(globals.contains("\"hasIndexedDB\":true"));

    let dom_result = engine
        .evaluate_script(
            "var canvas = document.createElement('canvas');
             var ctx = canvas.getContext('2d');
             ctx.fillRect(0, 0, 10, 10);
             ctx.beginPath();
             ctx.lineTo(1, 1);
             ctx.arc(5, 5, 2, 0, Math.PI);
             ctx.fill();
             ctx.stroke();
             var input = document.createElement('input');
             input.value = 'rv8';
             JSON.stringify({
               ctx: typeof ctx.fillRect === 'function',
               inputValue: input.value,
               canvasTag: canvas.tagName
             })",
        )
        .await
        .expect("dom apis");
    assert!(dom_result.contains("\"ctx\":true"));
    assert!(dom_result.contains("\"inputValue\":\"rv8\""));
    assert!(dom_result.contains("\"canvasTag\":\"CANVAS\""));

    let observer_result = engine
        .evaluate_script(
            "var el = document.createElement('div');
             var moSeen = 0;
             var mo = new MutationObserver(function() { moSeen++; });
             mo.observe(el, {});
             var io = new IntersectionObserver(function() {});
             io.observe(el);
             io.unobserve(el);
             io.disconnect();
             mo.disconnect();
             JSON.stringify({ moSeen: moSeen, records: mo.takeRecords().length })",
        )
        .await
        .expect("observers");
    assert!(observer_result.contains("\"moSeen\":0"));

    let idb_result = engine
        .evaluate_script(
            "var req = indexedDB.open('rv8_live_test', 1);
             var db = req.result;
             db.createObjectStore('items', { autoIncrement: true });
             var tx = db.transaction('items');
             var store = tx.objectStore('items');
             store.put({ hello: 'world' });
             var all = store.getAll();
             JSON.stringify({ storeLen: all.length, dbName: db.name })",
        )
        .await
        .expect("indexeddb");
    assert!(idb_result.contains("\"dbName\":\"rv8_live_test\""));

    let ws_result = engine
        .evaluate_script(
            "var ws = new WebSocket('wss://echo.websocket.events');
             ws.send('ping');
             ws.close();
             JSON.stringify({ readyState: ws.readyState, hasUrl: ws.url.length > 0 })",
        )
        .await
        .expect("websocket");
    assert!(ws_result.contains("\"hasUrl\":true"));

    let worker_result = engine
        .evaluate_script(
            "var w = new Worker('data:application/javascript,postMessage(1);');
             w.postMessage('hello');
             w.terminate();
             JSON.stringify({ spawned: w !== null })",
        )
        .await
        .expect("worker");
    assert!(worker_result.contains("\"spawned\":true"));

    let cdp = engine
        .cdp_send(r#"{"id":1,"method":"Runtime.evaluate","params":{"expression":"2+2"}}"#)
        .await
        .expect("cdp");
    assert!(cdp.contains("\"value\":4") || cdp.contains("\"value\": 4"));

    let frame = engine.capture_frame();
    assert!(frame.is_some(), "software-render path should produce a frame");
}
