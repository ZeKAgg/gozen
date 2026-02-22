import test from 'node:test';
import assert from 'node:assert/strict';

import {
  activateWithDeps,
  isGodotDocument,
  normalizeTrace,
  resetExtensionStateForTests,
  type ExtensionDeps,
  type LanguageClientLike,
} from '../extension';

type ConfigMap = Record<string, unknown>;

class FakeClient implements LanguageClientLike {
  public startCalls = 0;
  public stopCalls = 0;
  public traces: Array<'off' | 'messages' | 'verbose'> = [];

  async start(): Promise<void> {
    this.startCalls += 1;
  }

  async stop(): Promise<void> {
    this.stopCalls += 1;
  }

  setTrace(value: 'off' | 'messages' | 'verbose'): void {
    this.traces.push(value);
  }
}

interface Harness {
  deps: ExtensionDeps;
  context: { subscriptions: Array<{ dispose(): unknown }>; extensionUri: unknown };
  outputLines: string[];
  errorMessages: string[];
  infoMessages: string[];
  clients: FakeClient[];
  executeCalls: Array<{ command: string; args: unknown[] }>;
  fireConfigChange: (section: string) => void;
}

function createHarness(opts?: {
  config?: ConfigMap;
  commandExists?: boolean;
  gdFiles?: number;
  gdshaderFiles?: number;
  activeDoc?: { languageId: string; fileName: string } | undefined;
  errorChoice?: string | undefined;
}): Harness {
  const config = opts?.config ?? {};
  const outputLines: string[] = [];
  const errorMessages: string[] = [];
  const infoMessages: string[] = [];
  const executeCalls: Array<{ command: string; args: unknown[] }> = [];
  const clients: FakeClient[] = [];
  const configListeners: Array<(event: { affectsConfiguration(section: string): boolean }) => void> = [];

  const workspace = {
    getConfiguration: (_section: string) => ({
      get<T>(key: string, defaultValue: T): T {
        if (Object.prototype.hasOwnProperty.call(config, key)) {
          return config[key] as T;
        }
        return defaultValue;
      },
    }),
    async findFiles(pattern: string): Promise<ReadonlyArray<unknown>> {
      if (pattern === '**/*.gd') {
        return Array.from({ length: opts?.gdFiles ?? 0 }, (_v, i) => ({ i }));
      }
      if (pattern === '**/*.gdshader') {
        return Array.from({ length: opts?.gdshaderFiles ?? 0 }, (_v, i) => ({ i }));
      }
      return [];
    },
    createFileSystemWatcher: (_globPattern: string) => ({
      dispose(): void {},
    }),
    onDidChangeConfiguration(
      listener: (event: { affectsConfiguration(section: string): boolean }) => void,
    ) {
      configListeners.push(listener);
      return { dispose(): void {} };
    },
  };

  const window = {
    createOutputChannel: (_name: string) => ({
      appendLine(value: string): void {
        outputLines.push(value);
      },
      dispose(): void {},
    }),
    async showInformationMessage(message: string): Promise<unknown> {
      infoMessages.push(message);
      return undefined;
    },
    async showErrorMessage(message: string, ..._items: string[]): Promise<string | undefined> {
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
    registerCommand: (_command: string, _callback: () => Promise<void>) => ({
      dispose(): void {},
    }),
    async executeCommand(command: string, ...args: unknown[]): Promise<unknown> {
      executeCalls.push({ command, args });
      return undefined;
    },
  };

  const deps: ExtensionDeps = {
    workspace,
    window,
    commands,
    uri: {
      joinPath: (_base: unknown, ...pathSegments: string[]) => pathSegments.join('/'),
    },
    commandExists: (_command: string) => opts?.commandExists ?? true,
    createLanguageClient: (_id, _name, _serverOptions, _clientOptions) => {
      const client = new FakeClient();
      clients.push(client);
      return client;
    },
    stdioTransport: 0 as 0,
  };

  return {
    deps,
    context: { subscriptions: [], extensionUri: 'ext://uri' },
    outputLines,
    errorMessages,
    infoMessages,
    clients,
    executeCalls,
    fireConfigChange: (section: string) => {
      const event = {
        affectsConfiguration(candidate: string): boolean {
          return candidate === section;
        },
      };
      for (const listener of configListeners) {
        listener(event);
      }
    },
  };
}

void test('normalizeTrace maps invalid values to off', () => {
  assert.equal(normalizeTrace('off'), 'off');
  assert.equal(normalizeTrace('messages'), 'messages');
  assert.equal(normalizeTrace('verbose'), 'verbose');
  assert.equal(normalizeTrace('loud'), 'off');
});

void test('isGodotDocument detects by language and filename', () => {
  assert.equal(isGodotDocument(undefined), false);
  assert.equal(isGodotDocument({ languageId: 'gdscript', fileName: '/tmp/a.txt' }), true);
  assert.equal(isGodotDocument({ languageId: 'plaintext', fileName: '/tmp/a.gd' }), true);
  assert.equal(
    isGodotDocument({ languageId: 'plaintext', fileName: '/tmp/a.gdshader' }),
    true,
  );
  assert.equal(isGodotDocument({ languageId: 'plaintext', fileName: '/tmp/a.txt' }), false);
});

void test('activateWithDeps exits early when extension is disabled', async () => {
  resetExtensionStateForTests();
  const h = createHarness({ config: { enable: false } });
  await activateWithDeps(h.context, h.deps);
  assert.equal(h.clients.length, 0);
  assert.ok(
    h.outputLines.some((line) =>
      line.includes('Gozen extension is disabled by setting "gozen.enable".'),
    ),
  );
});

void test('activateWithDeps reports missing executable and does not start client', async () => {
  resetExtensionStateForTests();
  const h = createHarness({ config: { enable: true, path: 'missing-gozen' }, commandExists: false });
  await activateWithDeps(h.context, h.deps);
  assert.equal(h.clients.length, 0);
  assert.equal(h.errorMessages.length, 1);
  assert.ok(h.errorMessages[0].includes('Gozen executable not found'));
});

void test('activateWithDeps skips startup with no workspace files and non-Godot active doc', async () => {
  resetExtensionStateForTests();
  const h = createHarness({
    config: { enable: true },
    gdFiles: 0,
    gdshaderFiles: 0,
    activeDoc: { languageId: 'plaintext', fileName: '/tmp/readme.txt' },
  });
  await activateWithDeps(h.context, h.deps);
  assert.equal(h.clients.length, 0);
  assert.ok(
    h.outputLines.some((line) =>
      line.includes('No Godot files found in workspace. Skipping Gozen LSP startup.'),
    ),
  );
});

void test('activateWithDeps starts client and handles trace updates', async () => {
  resetExtensionStateForTests();
  const h = createHarness({
    config: { enable: true, 'trace.server': 'messages' },
    gdFiles: 1,
    gdshaderFiles: 0,
  });
  await activateWithDeps(h.context, h.deps);
  assert.equal(h.clients.length, 1);
  assert.equal(h.clients[0].startCalls, 1);
  assert.deepEqual(h.clients[0].traces, ['messages']);

  h.fireConfigChange('gozen.trace.server');
  assert.deepEqual(h.clients[0].traces, ['messages', 'messages']);
});
