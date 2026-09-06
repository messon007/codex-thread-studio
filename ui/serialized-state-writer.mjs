// Generated from ui-src; run npm run build:ui. Do not edit.
/** One independent FIFO per persistence domain; snapshot before joining the queue. */
export function createSerializedStateWriter(transport, onError) {
    let chain = Promise.resolve();
    function write(path, body, method = 'PUT') {
        const payload = JSON.stringify(body);
        const pending = chain.catch(() => { }).then(async () => {
            const response = await transport(path, {
                method,
                headers: { 'Content-Type': 'application/json' },
                body: payload,
            });
            if (!response.ok)
                throw new Error(`HTTP ${response.status}`);
        });
        chain = pending.catch(onError);
        return pending;
    }
    return { write };
}
