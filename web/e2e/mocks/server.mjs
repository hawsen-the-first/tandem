// Stand-in services for the end-to-end suite, started and stopped by dev/scripts/e2e.sh:
//   an OpenAI-compatible provider at /v1 with two models, and a SearXNG instance at /search.
// `capable` calls the forced echo tool and has 32,768 tokens of context; `weak` answers in plain
// text (so tool calling is unsupported) and has 4,096. No dependencies; listens on loopback only.
import { createServer } from 'node:http';

const PORT = Number(process.env['E2E_MOCK_PORT'] ?? '8092');

const MODELS = {
    object: 'list',
    data: [
        { id: 'capable', object: 'model', context_length: 32768 },
        { id: 'weak', object: 'model', context_length: 4096 },
    ],
};

const PLAIN = { choices: [{ message: { role: 'assistant', content: 'ok' } }] };
const TOOL = {
    choices: [
        {
            message: {
                role: 'assistant',
                content: null,
                tool_calls: [
                    {
                        id: '1',
                        type: 'function',
                        function: { name: 'echo', arguments: '{"text":"ok"}' },
                    },
                ],
            },
        },
    ],
};

function send(response, status, body) {
    const text = JSON.stringify(body);
    response.writeHead(status, {
        'content-type': 'application/json',
        'content-length': Buffer.byteLength(text),
    });
    response.end(text);
}

async function readJson(request) {
    const chunks = [];
    for await (const chunk of request) {
        chunks.push(chunk);
    }
    try {
        return JSON.parse(Buffer.concat(chunks).toString('utf8'));
    } catch {
        return {};
    }
}

const server = createServer((request, response) => {
    const url = new URL(request.url ?? '/', `http://127.0.0.1:${String(PORT)}`);
    if (request.method === 'GET' && url.pathname === '/v1/models') {
        send(response, 200, MODELS);
        return;
    }
    if (request.method === 'POST' && url.pathname === '/v1/chat/completions') {
        void readJson(request).then((body) => {
            if (body.model !== 'capable' && body.model !== 'weak') {
                send(response, 404, {
                    error: { code: 'model_not_found', message: 'no such model' },
                });
            } else if (body.model === 'capable' && Array.isArray(body.tools)) {
                send(response, 200, TOOL);
            } else {
                send(response, 200, PLAIN);
            }
        });
        return;
    }
    if (request.method === 'GET' && url.pathname === '/search') {
        send(response, 200, { results: [{ title: 'Tandem', url: 'https://example.com/tandem' }] });
        return;
    }
    if (url.pathname === '/healthz') {
        send(response, 200, { ok: true });
        return;
    }
    send(response, 404, { error: 'not found' });
});

server.listen(PORT, '127.0.0.1', () => {
    process.stdout.write(`e2e mocks listening on http://127.0.0.1:${String(PORT)}\n`);
});

for (const signal of ['SIGTERM', 'SIGINT']) {
    process.on(signal, () => {
        server.close(() => {
            process.exit(0);
        });
    });
}
