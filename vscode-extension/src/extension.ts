import { spawnSync } from 'child_process';
import type * as Vscode from 'vscode';
import type { LanguageClientOptions, ServerOptions } from 'vscode-languageclient/node';

let client: LanguageClientLike | undefined;
let starting = false;

interface DisposableLike {
  dispose(): void;
}

interface OutputChannelLike extends DisposableLike {
  appendLine(value: string): void;
}

interface ConfigurationChangeEventLike {
  affectsConfiguration(section: string): boolean;
}

interface WorkspaceLike {
  getConfiguration(section: string): {
    get<T>(key: string, defaultValue: T): T;
  };
  findFiles(
    pattern: string,
    exclude: string,
    maxResults: number,
  ): PromiseLike<ReadonlyArray<unknown>>;
  createFileSystemWatcher(globPattern: string): DisposableLike;
  onDidChangeConfiguration(
    listener: (event: ConfigurationChangeEventLike) => void,
  ): DisposableLike;
}

interface WindowLike {
  createOutputChannel(name: string): OutputChannelLike;
  showInformationMessage(message: string): PromiseLike<unknown>;
  showErrorMessage(message: string, ...items: string[]): PromiseLike<string | undefined>;
  activeTextEditor?: {
    document?: GodotDocumentLike;
  };
}

interface CommandsLike {
  registerCommand(command: string, callback: () => Promise<void>): DisposableLike;
  executeCommand(command: string, ...rest: unknown[]): PromiseLike<unknown>;
}

interface UriLike {
  joinPath(base: unknown, ...pathSegments: string[]): unknown;
}

export interface LanguageClientLike {
  start(): PromiseLike<unknown>;
  stop(): PromiseLike<unknown>;
  setTrace(value: 'off' | 'messages' | 'verbose'): PromiseLike<void> | void;
}

export interface ExtensionDeps {
  workspace: WorkspaceLike;
  window: WindowLike;
  commands: CommandsLike;
  uri: UriLike;
  commandExists: (command: string) => boolean;
  createLanguageClient: (
    id: string,
    name: string,
    serverOptions: ServerOptions,
    clientOptions: {
      documentSelector: Array<{ scheme: string; language?: string; pattern?: string }>;
      outputChannel: OutputChannelLike;
      synchronize: {
        fileEvents: DisposableLike;
      };
    },
  ) => LanguageClientLike;
  stdioTransport: number;
}

interface ExtensionContextLike {
  subscriptions: Array<{ dispose(): unknown }>;
  extensionUri: unknown;
}

export interface GodotDocumentLike {
  languageId: string;
  fileName: string;
}

export function normalizeTrace(value: string): 'off' | 'messages' | 'verbose' {
  if (value === 'messages' || value === 'verbose') {
    return value;
  }
  return 'off';
}

function commandExists(command: string): boolean {
  const lookupCommand = process.platform === 'win32' ? 'where' : 'which';
  const result = spawnSync(lookupCommand, [command], { encoding: 'utf8' });
  return result.status === 0;
}

async function hasGodotWorkspaceFiles(workspace: WorkspaceLike): Promise<boolean> {
  const excludes = '**/{.git,node_modules,target,out,dist}/**';
  const [gd, gdshader] = await Promise.all([
    workspace.findFiles('**/*.gd', excludes, 1),
    workspace.findFiles('**/*.gdshader', excludes, 1),
  ]);
  return gd.length > 0 || gdshader.length > 0;
}

export function isGodotDocument(doc: GodotDocumentLike | undefined): boolean {
  if (!doc) {
    return false;
  }
  if (doc.languageId === 'gdscript' || doc.languageId === 'gdshader') {
    return true;
  }
  return doc.fileName.endsWith('.gd') || doc.fileName.endsWith('.gdshader');
}

function createDefaultDeps(): ExtensionDeps {
  const vscodeMod = require('vscode') as typeof import('vscode');
  const languageClientMod = require('vscode-languageclient/node') as typeof import('vscode-languageclient/node');

  return {
    workspace: vscodeMod.workspace,
    window: vscodeMod.window,
    commands: vscodeMod.commands,
    uri: vscodeMod.Uri,
    commandExists,
    createLanguageClient: (id, name, serverOptions, clientOptions) =>
      new languageClientMod.LanguageClient(
        id,
        name,
        serverOptions,
        clientOptions as unknown as LanguageClientOptions,
      ) as unknown as LanguageClientLike,
    stdioTransport: languageClientMod.TransportKind.stdio,
  };
}

export async function activateWithDeps(
  context: ExtensionContextLike,
  deps: ExtensionDeps,
): Promise<void> {
  if (starting) {
    return;
  }
  starting = true;
  try {
    const cfg = deps.workspace.getConfiguration('gozen');
    const enabled = cfg.get<boolean>('enable', true);
    const outputChannel = deps.window.createOutputChannel('Gozen LSP');
    context.subscriptions.push(outputChannel);

    const restartCmd = deps.commands.registerCommand('gozen.restartLanguageServer', async () => {
      if (!client) {
        void deps.window.showInformationMessage(
          'Gozen language server is not running. Open a .gd or .gdshader file to start it.',
        );
        return;
      }
      await client.stop();
      await client.start();
    });
    context.subscriptions.push(restartCmd);

    if (!enabled) {
      outputChannel.appendLine('Gozen extension is disabled by setting "gozen.enable".');
      return;
    }

    const command = cfg.get<string>('path', 'gozen');
    if (!deps.commandExists(command)) {
      outputChannel.appendLine(`Could not resolve Gozen executable: ${command}`);
      const openSettings = 'Open Settings';
      const viewReadme = 'View README';
      const choice = await deps.window.showErrorMessage(
        `Gozen executable not found: "${command}". Configure "gozen.path" or add "gozen" to PATH.`,
        openSettings,
        viewReadme,
      );
      if (choice === openSettings) {
        await deps.commands.executeCommand('workbench.action.openSettings', 'gozen.path');
      } else if (choice === viewReadme) {
        const readmeUri = deps.uri.joinPath(context.extensionUri, 'README.md');
        await deps.commands.executeCommand('markdown.showPreview', readmeUri);
      }
      return;
    }

    const hasWorkspaceFiles = await hasGodotWorkspaceFiles(deps.workspace);
    if (!hasWorkspaceFiles && !isGodotDocument(deps.window.activeTextEditor?.document)) {
      outputChannel.appendLine('No Godot files found in workspace. Skipping Gozen LSP startup.');
      return;
    }

    const serverOptions: ServerOptions = {
      run: {
        command,
        args: ['lsp'],
        transport: deps.stdioTransport,
      },
      debug: {
        command,
        args: ['lsp'],
        transport: deps.stdioTransport,
        options: {
          env: {
            ...process.env,
            RUST_LOG: process.env.RUST_LOG || 'info',
          },
        },
      },
    };

    const clientOptions = {
      documentSelector: [
        { scheme: 'file', language: 'gdscript' },
        { scheme: 'file', pattern: '**/*.gd' },
        { scheme: 'file', pattern: '**/*.gdshader' },
      ],
      outputChannel,
      synchronize: {
        fileEvents: deps.workspace.createFileSystemWatcher('**/*.{gd,gdshader,tscn,tres,godot,json}'),
      },
    };

    if (!client) {
      client = deps.createLanguageClient('gozen', 'Gozen Language Server', serverOptions, clientOptions);
    }
    void client.setTrace(normalizeTrace(cfg.get<string>('trace.server', 'off')));
    await client.start();
    outputChannel.appendLine(`Started Gozen LSP using command: ${command} lsp`);

    const disposable = deps.workspace.onDidChangeConfiguration((e) => {
      if (e.affectsConfiguration('gozen.trace.server') && client) {
        const trace = deps.workspace.getConfiguration('gozen').get<string>('trace.server', 'off');
        void client.setTrace(normalizeTrace(trace));
        outputChannel.appendLine(`Updated server trace level: ${trace}`);
      }
    });
    context.subscriptions.push(disposable);
  } finally {
    starting = false;
  }
}

export async function activate(context: Vscode.ExtensionContext): Promise<void> {
  await activateWithDeps(context as unknown as ExtensionContextLike, createDefaultDeps());
}

export async function deactivate(): Promise<void> {
  if (!client) {
    return;
  }
  await client.stop();
  client = undefined;
}

export function resetExtensionStateForTests(): void {
  client = undefined;
  starting = false;
}
