"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const node_test_1 = __importDefault(require("node:test"));
const strict_1 = __importDefault(require("node:assert/strict"));
const extension_1 = require("../extension");
class FakeClient {
    constructor() {
        this.startCalls = 0;
        this.stopCalls = 0;
        this.traces = [];
    }
    async start() {
        this.startCalls += 1;
    }
    async stop() {
        this.stopCalls += 1;
    }
    setTrace(value) {
        this.traces.push(value);
    }
}
function createHarness(opts) {
    const config = opts?.config ?? {};
    const outputLines = [];
    const errorMessages = [];
    const infoMessages = [];
    const executeCalls = [];
    const clients = [];
    const configListeners = [];
    const workspace = {
        getConfiguration: (_section) => ({
            get(key, defaultValue) {
                if (Object.prototype.hasOwnProperty.call(config, key)) {
                    return config[key];
                }
                return defaultValue;
            },
        }),
        async findFiles(pattern) {
            if (pattern === '**/*.gd') {
                return Array.from({ length: opts?.gdFiles ?? 0 }, (_v, i) => ({ i }));
            }
            if (pattern === '**/*.gdshader') {
                return Array.from({ length: opts?.gdshaderFiles ?? 0 }, (_v, i) => ({ i }));
            }
            return [];
        },
        createFileSystemWatcher: (_globPattern) => ({
            dispose() { },
        }),
        onDidChangeConfiguration(listener) {
            configListeners.push(listener);
            return { dispose() { } };
        },
    };
    const window = {
        createOutputChannel: (_name) => ({
            appendLine(value) {
                outputLines.push(value);
            },
            dispose() { },
        }),
        async showInformationMessage(message) {
            infoMessages.push(message);
            return undefined;
        },
        async showErrorMessage(message, ..._items) {
            errorMessages.push(message);
            return opts?.errorChoice;
        },
        activeTextEditor: opts?.activeDoc
            ? {
                document: opts.activeDoc,
            }
            : undefined,
    };
    const commands = {
        registerCommand: (_command, _callback) => ({
            dispose() { },
        }),
        async executeCommand(command, ...args) {
            executeCalls.push({ command, args });
            return undefined;
        },
    };
    const deps = {
        workspace,
        window,
        commands,
        uri: {
            joinPath: (_base, ...pathSegments) => pathSegments.join('/'),
        },
        commandExists: (_command) => opts?.commandExists ?? true,
        createLanguageClient: (_id, _name, _serverOptions, _clientOptions) => {
            const client = new FakeClient();
            clients.push(client);
            return client;
        },
        stdioTransport: 0,
    };
    return {
        deps,
        context: { subscriptions: [], extensionUri: 'ext://uri' },
        outputLines,
        errorMessages,
        infoMessages,
        clients,
        executeCalls,
        fireConfigChange: (section) => {
            const event = {
                affectsConfiguration(candidate) {
                    return candidate === section;
                },
            };
            for (const listener of configListeners) {
                listener(event);
            }
        },
    };
}
void (0, node_test_1.default)('normalizeTrace maps invalid values to off', () => {
    strict_1.default.equal((0, extension_1.normalizeTrace)('off'), 'off');
    strict_1.default.equal((0, extension_1.normalizeTrace)('messages'), 'messages');
    strict_1.default.equal((0, extension_1.normalizeTrace)('verbose'), 'verbose');
    strict_1.default.equal((0, extension_1.normalizeTrace)('loud'), 'off');
});
void (0, node_test_1.default)('isGodotDocument detects by language and filename', () => {
    strict_1.default.equal((0, extension_1.isGodotDocument)(undefined), false);
    strict_1.default.equal((0, extension_1.isGodotDocument)({ languageId: 'gdscript', fileName: '/tmp/a.txt' }), true);
    strict_1.default.equal((0, extension_1.isGodotDocument)({ languageId: 'plaintext', fileName: '/tmp/a.gd' }), true);
    strict_1.default.equal((0, extension_1.isGodotDocument)({ languageId: 'plaintext', fileName: '/tmp/a.gdshader' }), true);
    strict_1.default.equal((0, extension_1.isGodotDocument)({ languageId: 'plaintext', fileName: '/tmp/a.txt' }), false);
});
void (0, node_test_1.default)('activateWithDeps exits early when extension is disabled', async () => {
    (0, extension_1.resetExtensionStateForTests)();
    const h = createHarness({ config: { enable: false } });
    await (0, extension_1.activateWithDeps)(h.context, h.deps);
    strict_1.default.equal(h.clients.length, 0);
    strict_1.default.ok(h.outputLines.some((line) => line.includes('Gozen extension is disabled by setting "gozen.enable".')));
});
void (0, node_test_1.default)('activateWithDeps reports missing executable and does not start client', async () => {
    (0, extension_1.resetExtensionStateForTests)();
    const h = createHarness({ config: { enable: true, path: 'missing-gozen' }, commandExists: false });
    await (0, extension_1.activateWithDeps)(h.context, h.deps);
    strict_1.default.equal(h.clients.length, 0);
    strict_1.default.equal(h.errorMessages.length, 1);
    strict_1.default.ok(h.errorMessages[0].includes('Gozen executable not found'));
});
void (0, node_test_1.default)('activateWithDeps skips startup with no workspace files and non-Godot active doc', async () => {
    (0, extension_1.resetExtensionStateForTests)();
    const h = createHarness({
        config: { enable: true },
        gdFiles: 0,
        gdshaderFiles: 0,
        activeDoc: { languageId: 'plaintext', fileName: '/tmp/readme.txt' },
    });
    await (0, extension_1.activateWithDeps)(h.context, h.deps);
    strict_1.default.equal(h.clients.length, 0);
    strict_1.default.ok(h.outputLines.some((line) => line.includes('No Godot files found in workspace. Skipping Gozen LSP startup.')));
});
void (0, node_test_1.default)('activateWithDeps starts client and handles trace updates', async () => {
    (0, extension_1.resetExtensionStateForTests)();
    const h = createHarness({
        config: { enable: true, 'trace.server': 'messages' },
        gdFiles: 1,
        gdshaderFiles: 0,
    });
    await (0, extension_1.activateWithDeps)(h.context, h.deps);
    strict_1.default.equal(h.clients.length, 1);
    strict_1.default.equal(h.clients[0].startCalls, 1);
    strict_1.default.deepEqual(h.clients[0].traces, ['messages']);
    h.fireConfigChange('gozen.trace.server');
    strict_1.default.deepEqual(h.clients[0].traces, ['messages', 'messages']);
});
//# sourceMappingURL=extension.unit.test.js.map